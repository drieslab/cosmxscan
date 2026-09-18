test_that("the scanner emits exactly the nonzero triplets", {
    truth <- fixture_dense()
    nz <- which(truth$feats != 0, arr.ind = TRUE)
    expected <- data.frame(row_id = as.integer(nz[, 1]),
                           col_id = as.integer(nz[, 2]),
                           value  = as.numeric(truth$feats[nz]))
    expected <- expected[order(expected$row_id, expected$col_id), ]

    got <- drain(CosmxReader$new(fixture_path(), 2L), 10L)
    ord <- order(got$row_id, got$col_id)

    expect_equal(as.integer(got$row_id[ord]), expected$row_id)
    expect_equal(as.integer(got$col_id[ord]), expected$col_id)
    expect_equal(as.numeric(got$value[ord]), expected$value)
})

test_that("an all-zero cell contributes no triplets but still advances row_id", {
    truth <- fixture_dense()
    zero_rows <- which(rowSums(truth$feats) == 0)
    expect_gt(length(zero_rows), 0)   # the fixture must actually contain one

    got <- drain(CosmxReader$new(fixture_path(), 2L), 10L)
    expect_false(any(got$row_id %in% zero_rows))
    # row ids stay aligned to file order, so the rows after the gap are intact
    expect_equal(max(got$row_id), nrow(truth$feats))
})

test_that("fov and cell_ID come back one per cell, in file order", {
    truth <- fixture_dense()
    got <- drain(CosmxReader$new(fixture_path(), 10L), 10L)
    expect_equal(as.integer(got$fov), as.integer(truth$fov))
    expect_equal(as.integer(got$cell_ID), as.integer(truth$cell_ID))
    # cell_ID restarts per fov, which is why a global id needs both
    expect_true(anyDuplicated(got$cell_ID) > 0)
})

test_that("batching does not change the result", {
    one <- drain(CosmxReader$new(fixture_path(), 2L), 100L)
    many <- drain(CosmxReader$new(fixture_path(), 2L), 1L)

    expect_gt(many$n_batches, one$n_batches)
    for (f in c("row_id", "col_id", "value", "fov", "cell_ID")) {
        expect_equal(one[[f]], many[[f]], info = f)
    }
})

test_that("gzipped and plain input agree", {
    gz <- drain(CosmxReader$new(fixture_path(gz = TRUE), 2L), 10L)
    plain <- drain(CosmxReader$new(fixture_path(gz = FALSE), 2L), 10L)
    for (f in c("row_id", "col_id", "value", "fov", "cell_ID")) {
        expect_equal(gz[[f]], plain[[f]], info = f)
    }
})

test_that("eof and rows_done report the end of the file", {
    reader <- CosmxReader$new(fixture_path(), 2L)
    chunk <- reader$next_chunk(100L)
    expect_true(isTRUE(chunk$eof))
    expect_equal(as.integer(chunk$n_rows), nrow(fixture_dense()$feats))
    expect_equal(as.integer(reader$rows_done()), nrow(fixture_dense()$feats))
    expect_equal(as.integer(reader$next_chunk(100L)$n_rows), 0L)
    reader$close()
})

test_that("n_nz counts the triplets in the batch", {
    reader <- CosmxReader$new(fixture_path(), 2L)
    chunk <- reader$next_chunk(100L)
    expect_equal(as.numeric(chunk$n_nz), length(chunk$value))
    reader$close()
})

test_that("skip_cols offsets col_id and nothing else", {
    # fields 1 and 2 are always read as fov and cell_ID; skip_cols only moves
    # the origin of the emitted feature index, so the triplets are unchanged
    default <- drain(CosmxReader$new(fixture_path(), 2L), 10L)
    shifted <- drain(CosmxReader$new(fixture_path(), 1L), 10L)

    expect_equal(shifted$value, default$value)
    expect_equal(as.integer(shifted$col_id), as.integer(default$col_id) + 1L)
    expect_equal(as.integer(shifted$fov), as.integer(default$fov))
    expect_equal(as.integer(shifted$cell_ID), as.integer(default$cell_ID))
})

test_that("col_id is 1-based for the documented skip_cols = 2", {
    truth <- fixture_dense()
    got <- drain(CosmxReader$new(fixture_path(), 2L), 10L)
    expect_equal(min(got$col_id), 1L)
    expect_equal(max(got$col_id), ncol(truth$feats))
})

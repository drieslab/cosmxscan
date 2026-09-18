# The fixture is 6 cells by 4 features, laid out like a real exprMat_file:
# two leading non-feature columns (fov, cell_ID) then one column per gene.
# Row 2 is all zero, and cell_ID restarts at 1 in the second fov, so the
# fixture exercises both of the cases that make CosMx awkward.
fixture_path <- function(gz = TRUE) {
    system.file("extdata",
                if (gz) "mini_exprMat.csv.gz" else "mini_exprMat.csv",
                package = "cosmxscan", mustWork = TRUE)
}

# Dense truth, read with base R rather than the scanner, so the expectations
# below are independent of the thing under test.
fixture_dense <- function() {
    m <- as.matrix(utils::read.csv(fixture_path(gz = FALSE)))
    list(fov = m[, 1], cell_ID = m[, 2], feats = m[, -(1:2), drop = FALSE])
}

# Drain a reader completely, concatenating every batch.
drain <- function(reader, max_rows) {
    out <- list(row_id = integer(), col_id = integer(), value = numeric(),
                fov = integer(), cell_ID = integer(), n_batches = 0L)
    repeat {
        chunk <- reader$next_chunk(max_rows)
        if (chunk$n_rows == 0L) break
        out$row_id  <- c(out$row_id, chunk$row_id)
        out$col_id  <- c(out$col_id, chunk$col_id)
        out$value   <- c(out$value, chunk$value)
        out$fov     <- c(out$fov, chunk$fov)
        out$cell_ID <- c(out$cell_ID, chunk$cell_ID)
        out$n_batches <- out$n_batches + 1L
        if (isTRUE(chunk$eof)) break
    }
    reader$close()
    out
}

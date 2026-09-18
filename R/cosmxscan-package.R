#' cosmxscan: Rust Byte Scanner for CosMx Expression Matrices
#'
#' A NanoString CosMx whole-transcriptome `exprMat_file` is roughly 20,000
#' columns wide and about 5 percent dense, so most of the file is zeros that a
#' general CSV reader has to tokenize before discarding. cosmxscan walks the
#' gzipped file as bytes and emits only the nonzero entries, as
#' `(row_id, col_id, value)` triplets together with the `fov` and `cell_ID`
#' columns needed to reconstruct globally unique cell identifiers.
#'
#' The package has no R dependencies and knows nothing about Giotto. It
#' exposes a single object, [CosmxReader], which is a stateful iterator: open
#' a file, pull batches until end of file, then close. Peak memory is one
#' batch, so a matrix far larger than RAM can be streamed into any consumer.
#'
#' @section Measured on slide S0 (493,834 cells by 20,378 features):
#' \tabular{lr}{
#'   cosmxscan (this package) \tab 100 s \cr
#'   equivalent C++ byte scanner \tab 141 s \cr
#'   general wide-CSV reader \tab 974 s \cr
#' }
#'
#' @section Building:
#' A Rust toolchain is required at install time; see `SystemRequirements` in
#' DESCRIPTION. `flate2` uses the pure-Rust `miniz_oxide` backend by default,
#' so there is no system zlib to link against.
#'
#' @keywords internal
"_PACKAGE"

# cosmxscan

A Rust byte scanner for NanoString CosMx `exprMat_file` count matrices,
exposed to R through [extendr](https://extendr.github.io/).

A CosMx whole-transcriptome export is about 20,000 columns wide and roughly
5 percent dense, so most of the file is zeros. A general CSV reader tokenizes
all of them before throwing them away. cosmxscan walks the gzipped file as
bytes and emits only the nonzeros.

Measured on slide S0, 493,834 cells by 20,378 features:

| reader | time |
|---|---:|
| cosmxscan | 100 s |
| equivalent C++ byte scanner | 141 s |
| general wide-CSV reader | 974 s |

## Installation

Requires a Rust toolchain (`cargo`, `rustc`). `flate2` uses the pure-Rust
`miniz_oxide` backend, so no system zlib is needed.

```r
remotes::install_github("drieslab/cosmxscan")
```

## Usage

`CosmxReader` is a stateful iterator. Open a file, pull batches until end of
file, then close. Peak memory is one batch, so the matrix may be far larger
than RAM.

```r
library(cosmxscan)

reader <- CosmxReader$new("S0_exprMat_file.csv.gz", skip_cols = 2L)

repeat {
    chunk <- reader$next_chunk(max_rows = 10000L)
    if (chunk$n_rows == 0L) break
    # chunk$row_id, chunk$col_id, chunk$value  nonzero triplets
    # chunk$fov,    chunk$cell_ID              one entry per cell in the batch
    if (isTRUE(chunk$eof)) break
}

reader$close()
```

`skip_cols` is the number of leading non-feature columns; CosMx ships `fov`
and `cell_ID`, so it is 2. Feature identifiers come from the header line,
which the caller reads separately.

### Fields returned by `next_chunk()`

| field | description |
|---|---|
| `row_id`, `col_id`, `value` | nonzero triplets, 1-based, row ids global across batches |
| `fov`, `cell_ID` | one entry per cell in this batch |
| `n_rows`, `n_nz` | cells and nonzeros in this batch |
| `offset` | compressed byte position, for progress reporting |
| `eof` | `TRUE` once the end of the file has been reached |

The matrix's own `cell_ID` restarts within each field of view, so a globally
unique identifier has to be composed from both columns, conventionally as
`c_<slide>_<fov>_<cell_ID>`.

## Relationship to Giotto

cosmxscan has no Giotto dependency and can be used on its own. GiottoDisk can
use it as an optional fast path for CosMx ingestion, listed under `Suggests`
because of the Rust build requirement.

## License

MIT

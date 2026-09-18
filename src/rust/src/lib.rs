// ===========================================================================
// Wide CosMx exprMat -> COO triplets, streaming. Rust/extendr port.
// ===========================================================================
//
// A faithful port of .SCANNER_SRC (convenience-cosmx.R:472-611). Same
// algorithm, same emitted fields, same chunking contract, so the two can be
// diffed triplet-for-triplet.
//
// Emits exactly what a parquetExprStore holds:
//   row_id  data line number, 1-based, file order
//   col_id  field position minus skip_cols, 1-based
//   value   the count, nonzeros only
// plus (fov, cell_ID) per row so global ids compose without a second pass,
// and the compressed byte offset for a progress bar.
//
// TWO DIFFERENCES FROM THE C++, both in Rust's favour:
//   * inflate is miniz_oxide (pure Rust, via flate2's default features), so
//     there is NO system zlib to link -- the C++ needs PKG_LIBS="-lz".
//   * bounds are checked; a malformed row cannot walk off the buffer.
//
// Unchanged: a persistent handle rather than a byte offset (gzip cannot seek
// cheaply, so offset-resume would be quadratic), and chunks that always end on
// a newline so parser state never crosses one.

use extendr_api::prelude::*;
use flate2::read::MultiGzDecoder;
use std::fs::File;
use std::io::{BufReader, Read};

const BUF_BYTES: usize = 1 << 22; // 4 MB, matching the C++ reader
const IN_BYTES: usize = 1 << 20;  // 1 MB, matching gzbuffer()

/// Counts bytes pulled from the underlying file, so `offset` reports
/// COMPRESSED progress the way gzoffset() does.
struct Counting<R> {
    inner: R,
    count: u64,
}
impl<R: Read> Read for Counting<R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(b)?;
        self.count += n as u64;
        Ok(n)
    }
}

enum Src {
    Gz(Box<MultiGzDecoder<Counting<BufReader<File>>>>),
    Plain(Box<Counting<BufReader<File>>>),
}

impl Src {
    fn fill(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Src::Gz(r) => r.read(b),
            Src::Plain(r) => r.read(b),
        }
    }
    fn compressed_pos(&self) -> f64 {
        match self {
            Src::Gz(r) => r.get_ref().count as f64,
            Src::Plain(r) => r.count as f64,
        }
    }
}

/// Streaming reader over a wide CosMx exprMat CSV (.csv or .csv.gz).
///
/// `#[extendr]` is required on the STRUCT as well as on the impl block below.
/// Without it extendr generates no external-pointer conversions and the impl
/// fails to compile with "the trait bound `CosmxReader: ToVectorValue` is not
/// satisfied" -- it falls back to treating the struct as a value to coerce
/// into an R vector. See extendr-api's own altrep_tests.rs: "need to make the
/// object `.into_robj()`-able".
#[extendr]
pub struct CosmxReader {
    src: Option<Src>,
    buf: Vec<u8>,
    len: usize,
    pos: usize,
    header_done: bool,
    eof: bool,
    skip_cols: i32,
    rows_total: i32,
}

impl CosmxReader {
    /// Refill the scan buffer. Loops because Read may return short.
    fn refill(&mut self) -> bool {
        let buf = &mut self.buf;
        let src = match self.src.as_mut() {
            Some(s) => s,
            None => return false,
        };
        let mut total = 0usize;
        while total < buf.len() {
            match src.fill(&mut buf[total..]) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) => panic!("read error: {}", e),
            }
        }
        self.len = total;
        self.pos = 0;
        if total == 0 {
            self.eof = true;
            false
        } else {
            true
        }
    }
}

#[extendr]
impl CosmxReader {
    /// Open a scanner. `.gz` is detected by extension and inflated inline.
    fn new(path: &str, skip_cols: i32) -> Self {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) => panic!("cannot open {}: {}", path, e),
        };
        let counted = Counting {
            inner: BufReader::with_capacity(IN_BYTES, f),
            count: 0,
        };
        let src = if path.to_ascii_lowercase().ends_with(".gz") {
            Src::Gz(Box::new(MultiGzDecoder::new(counted)))
        } else {
            Src::Plain(Box::new(counted))
        };
        Self {
            src: Some(src),
            buf: vec![0u8; BUF_BYTES],
            len: 0,
            pos: 0,
            header_done: false,
            eof: false,
            skip_cols,
            rows_total: 0,
        }
    }

    /// Scan up to `max_rows` data rows, returning their nonzero triplets.
    fn next_chunk(&mut self, max_rows: i32) -> List {
        let max_rows = if max_rows < 1 { 1 } else { max_rows };
        let cap = 12_000_000usize; // same reserve as the C++ reader
        let mut rid: Vec<i32> = Vec::with_capacity(cap);
        let mut cid: Vec<i32> = Vec::with_capacity(cap);
        let mut val: Vec<f64> = Vec::with_capacity(cap);
        let mut fov: Vec<i32> = Vec::with_capacity(max_rows as usize);
        let mut cell: Vec<i32> = Vec::with_capacity(max_rows as usize);

        let mut rows: i32 = 0;
        let mut field: i32 = 0;
        let mut cur_fov: i32 = 0;
        let mut cur_cell: i32 = 0;
        let mut acc: i64 = 0;
        let mut neg = false;
        let mut done = false;

        while !done {
            if self.pos >= self.len && !self.refill() {
                break; // EOF
            }
            while self.pos < self.len {
                let c = self.buf[self.pos];

                if c == b',' || c == b'\n' || c == b'\r' {
                    if !self.header_done {
                        if c == b'\n' {
                            self.header_done = true;
                            field = 0;
                        }
                        self.pos += 1;
                        continue;
                    }
                    field += 1;
                    let signed = if neg { -acc } else { acc };
                    if field == 1 {
                        cur_fov = signed as i32;
                    } else if field == 2 {
                        cur_cell = signed as i32;
                    } else if acc != 0 {
                        rid.push(self.rows_total + rows + 1);
                        cid.push(field - self.skip_cols);
                        val.push(signed as f64);
                    }
                    acc = 0;
                    neg = false;

                    if c == b'\n' {
                        if field > 1 {
                            // blank lines ignored
                            rows += 1;
                            fov.push(cur_fov);
                            cell.push(cur_cell);
                        }
                        field = 0;
                        if rows >= max_rows {
                            self.pos += 1;
                            done = true;
                            break;
                        }
                    }
                    self.pos += 1;
                    continue;
                }

                if !self.header_done {
                    self.pos += 1;
                    continue;
                }

                if c.is_ascii_digit() {
                    acc = acc * 10 + (c - b'0') as i64;
                } else if c == b'-' {
                    neg = true;
                } else {
                    panic!(
                        "unexpected character '{}' at data row {} field {} -- \
                         this scanner assumes integer counts",
                        c as char,
                        self.rows_total + rows + 1,
                        field + 1
                    );
                }
                self.pos += 1;
            }
        }

        // a final row with no trailing newline
        if !done && field > 0 {
            field += 1;
            if field > self.skip_cols && acc != 0 {
                rid.push(self.rows_total + rows + 1);
                cid.push(field - self.skip_cols);
                val.push(if neg { -acc } else { acc } as f64);
            }
            rows += 1;
            fov.push(cur_fov);
            cell.push(cur_cell);
        }
        self.rows_total += rows;

        let nnz = rid.len() as f64;
        let offset = self
            .src
            .as_ref()
            .map(|s| s.compressed_pos())
            .unwrap_or(0.0);
        let at_eof = self.eof && rows < max_rows;

        list!(
            row_id = rid,
            col_id = cid,
            value = val,
            fov = fov,
            cell_ID = cell,
            n_rows = rows,
            n_nz = nnz,
            offset = offset,
            eof = at_eof
        )
    }

    /// Release the file handle.
    fn close(&mut self) {
        self.src = None;
    }

    /// Data rows consumed so far.
    fn rows_done(&self) -> i32 {
        self.rows_total
    }
}

// NOTE: the module-declaration macro is deliberately absent from this file.
//
// rextendr::rust_source() appends a generated one (mod rextendr, exposing
// impl CosmxReader). It decides whether to do so by TEXT-MATCHING the macro's
// name against the source -- so even a mention inside a comment suppresses
// generation, the symbol wrap__make_rextendr_wrappers is never emitted, and
// the build fails at the wrapper .Call() with the crate having compiled fine.
// That is why the macro name is not spelled out anywhere in this file.
//
// A PACKAGE build does need it, declared explicitly: the macro takes the crate
// module name (cosmxrust) followed by `impl CosmxReader;`.

extendr_module! {
    mod cosmxscan;
    impl CosmxReader;
}

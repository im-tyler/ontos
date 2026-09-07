use std::io::{self, Read, Write};

pub const MAGIC: &[u8; 4] = b"ONTO";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Record {
    Header {
        world_w: u32,
        world_h: u32,
    },
    TickHeader {
        tick: u64,
    },
    Snapshot {
        population: u64,
    },
    CellFlipped {
        tick: u64,
        x: u32,
        y: u32,
    },
    RegionLevel {
        region_x: u32,
        region_y: u32,
        level: u8,
    },
    RegionState {
        tick: u64,
        region_x: u32,
        region_y: u32,
        level: u8,
        population: u64,
        hash: u64,
    },
}

pub struct StreamWriter<W: Write> {
    out: W,
}

impl<W: Write> StreamWriter<W> {
    pub fn new(mut out: W, world_w: u32, world_h: u32) -> io::Result<Self> {
        out.write_all(MAGIC)?;
        out.write_all(&FORMAT_VERSION.to_le_bytes())?;
        out.write_all(&world_w.to_le_bytes())?;
        out.write_all(&world_h.to_le_bytes())?;
        Ok(StreamWriter { out })
    }

    pub fn write(&mut self, record: &Record) -> io::Result<()> {
        match record {
            Record::Header { .. } => Ok(()),
            Record::TickHeader { tick } => {
                self.out.write_all(&[1u8])?;
                self.out.write_all(&tick.to_le_bytes())
            }
            Record::Snapshot { population } => {
                self.out.write_all(&[2u8])?;
                self.out.write_all(&population.to_le_bytes())
            }
            Record::CellFlipped { tick, x, y } => {
                self.out.write_all(&[3u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&x.to_le_bytes())?;
                self.out.write_all(&y.to_le_bytes())
            }
            Record::RegionLevel {
                region_x,
                region_y,
                level,
            } => {
                self.out.write_all(&[4u8])?;
                self.out.write_all(&region_x.to_le_bytes())?;
                self.out.write_all(&region_y.to_le_bytes())?;
                self.out.write_all(&[*level])
            }
            Record::RegionState {
                tick,
                region_x,
                region_y,
                level,
                population,
                hash,
            } => {
                self.out.write_all(&[5u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&region_x.to_le_bytes())?;
                self.out.write_all(&region_y.to_le_bytes())?;
                self.out.write_all(&[*level])?;
                self.out.write_all(&population.to_le_bytes())?;
                self.out.write_all(&hash.to_le_bytes())
            }
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

#[derive(Debug)]
pub enum ParseError {
    BadMagic,
    BadVersion(u32),
    UnknownTag(u8),
    Truncated,
    BadLevel(u8),
    Io(io::Error),
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> Self {
        ParseError::Io(e)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::BadMagic => write!(f, "bad magic"),
            ParseError::BadVersion(v) => write!(f, "unsupported format version {v}"),
            ParseError::UnknownTag(t) => write!(f, "unknown record tag {t}"),
            ParseError::Truncated => write!(f, "truncated record"),
            ParseError::BadLevel(l) => write!(f, "invalid level byte {l}"),
            ParseError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl PartialEq for ParseError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParseError::BadMagic, ParseError::BadMagic) => true,
            (ParseError::BadVersion(a), ParseError::BadVersion(b)) => a == b,
            (ParseError::UnknownTag(a), ParseError::UnknownTag(b)) => a == b,
            (ParseError::Truncated, ParseError::Truncated) => true,
            (ParseError::BadLevel(a), ParseError::BadLevel(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct StreamReader<R: Read> {
    input: R,
    header: Option<(u32, u32)>,
    scratch: [u8; 33],
}

impl<R: Read> StreamReader<R> {
    pub fn new(mut input: R) -> Result<Self, ParseError> {
        let mut buf = [0u8; 16];
        read_exact(&mut input, &mut buf)?;
        if &buf[0..4] != MAGIC {
            return Err(ParseError::BadMagic);
        }
        let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(ParseError::BadVersion(version));
        }
        let world_w = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let world_h = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        Ok(StreamReader {
            input,
            header: Some((world_w, world_h)),
            scratch: [0u8; 33],
        })
    }

    pub fn header(&self) -> (u32, u32) {
        self.header.expect("header consumed in new")
    }

    pub fn next_record(&mut self) -> Result<Option<Record>, ParseError> {
        let mut tag = [0u8; 1];
        match self.input.read_exact(&mut tag) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(ParseError::Io(e)),
        }
        let record = match tag[0] {
            1 => {
                self.take(8)?;
                Record::TickHeader {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                }
            }
            2 => {
                self.take(8)?;
                Record::Snapshot {
                    population: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                }
            }
            3 => {
                self.take(16)?;
                Record::CellFlipped {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    x: u32::from_le_bytes(self.scratch[8..12].try_into().unwrap()),
                    y: u32::from_le_bytes(self.scratch[12..16].try_into().unwrap()),
                }
            }
            4 => {
                self.take(9)?;
                let level = self.scratch[8];
                if level > 1 {
                    return Err(ParseError::BadLevel(level));
                }
                Record::RegionLevel {
                    region_x: u32::from_le_bytes(self.scratch[0..4].try_into().unwrap()),
                    region_y: u32::from_le_bytes(self.scratch[4..8].try_into().unwrap()),
                    level,
                }
            }
            5 => {
                self.take(33)?;
                let level = self.scratch[16];
                if level > 1 {
                    return Err(ParseError::BadLevel(level));
                }
                Record::RegionState {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    region_x: u32::from_le_bytes(self.scratch[8..12].try_into().unwrap()),
                    region_y: u32::from_le_bytes(self.scratch[12..16].try_into().unwrap()),
                    level,
                    population: u64::from_le_bytes(self.scratch[17..25].try_into().unwrap()),
                    hash: u64::from_le_bytes(self.scratch[25..33].try_into().unwrap()),
                }
            }
            t => return Err(ParseError::UnknownTag(t)),
        };
        Ok(Some(record))
    }

    fn take(&mut self, len: usize) -> Result<(), ParseError> {
        read_exact(&mut self.input, &mut self.scratch[..len])
    }
}

fn read_exact<R: Read>(input: &mut R, buf: &mut [u8]) -> Result<(), ParseError> {
    input.read_exact(buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            ParseError::Truncated
        } else {
            ParseError::Io(e)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
            w.write(&Record::Snapshot { population: 5 }).unwrap();
            w.write(&Record::CellFlipped {
                tick: 1,
                x: 3,
                y: 4,
            })
            .unwrap();
            w.write(&Record::RegionLevel {
                region_x: 1,
                region_y: 0,
                level: 0,
            })
            .unwrap();
            w.flush().unwrap();
        }
        assert_eq!(&buf[0..4], MAGIC);
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(buf[8..12].try_into().unwrap()), 128);
        assert_eq!(u32::from_le_bytes(buf[12..16].try_into().unwrap()), 128);
        assert_eq!(buf[16], 1);
        assert_eq!(buf[25], 2);
        assert_eq!(buf[34], 3);
        assert_eq!(buf[51], 4);
        assert_eq!(buf.len(), 61);
    }

    fn reader(buf: &[u8]) -> StreamReader<&[u8]> {
        StreamReader::new(buf).unwrap()
    }

    #[test]
    fn parse_roundtrip() {
        let mut buf = Vec::new();
        let records = [
            Record::TickHeader { tick: 7 },
            Record::Snapshot { population: 1234 },
            Record::CellFlipped {
                tick: 7,
                x: 9,
                y: 10,
            },
            Record::RegionLevel {
                region_x: 1,
                region_y: 0,
                level: 0,
            },
            Record::RegionState {
                tick: 7,
                region_x: 1,
                region_y: 0,
                level: 0,
                population: 3,
                hash: 0xdeadbeef,
            },
        ];
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            for r in &records {
                w.write(r).unwrap();
            }
            w.flush().unwrap();
        }
        let mut r = reader(&buf);
        assert_eq!(r.header(), (128, 128));
        for rec in &records {
            assert_eq!(r.next_record().unwrap(), Some(*rec));
        }
        assert_eq!(r.next_record().unwrap(), None);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
        }
        buf[0] = b'X';
        assert_eq!(
            StreamReader::new(&buf[..]).unwrap_err(),
            ParseError::BadMagic
        );
    }

    #[test]
    fn rejects_bad_version() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
        }
        buf[4] = 9;
        assert_eq!(
            StreamReader::new(&buf[..]).unwrap_err(),
            ParseError::BadVersion(9)
        );
    }

    #[test]
    fn rejects_unknown_tag() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
        }
        buf[16] = 0xff;
        assert_eq!(
            reader(&buf).next_record().unwrap_err(),
            ParseError::UnknownTag(0xff)
        );
    }

    #[test]
    fn rejects_truncated_payload() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
        }
        buf.truncate(18);
        assert_eq!(
            reader(&buf).next_record().unwrap_err(),
            ParseError::Truncated
        );
    }

    #[test]
    fn rejects_truncated_header() {
        let buf = [0u8; 10];
        assert_eq!(
            StreamReader::new(&buf[..]).unwrap_err(),
            ParseError::Truncated
        );
    }

    #[test]
    fn rejects_bad_level_byte() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::RegionLevel {
                region_x: 0,
                region_y: 0,
                level: 2,
            })
            .unwrap();
        }
        assert_eq!(
            reader(&buf).next_record().unwrap_err(),
            ParseError::BadLevel(2)
        );
    }

    #[test]
    fn rejects_trailing_garbage() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new(&mut buf, 128, 128).unwrap();
            w.write(&Record::TickHeader { tick: 0 }).unwrap();
        }
        buf.push(1);
        buf.push(2);
        let mut r = reader(&buf);
        assert_eq!(
            r.next_record().unwrap(),
            Some(Record::TickHeader { tick: 0 })
        );
        assert_eq!(r.next_record().unwrap_err(), ParseError::Truncated);
    }
}

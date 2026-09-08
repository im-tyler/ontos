use std::io::{self, Read, Write};

pub const MAGIC: &[u8; 4] = b"ONTO";
pub const FORMAT_VERSION: u32 = 1;
pub const FORMAT_VERSION_GRAVITY: u32 = 2;

#[derive(Clone, Copy, PartialEq, Debug)]
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
    BodyState {
        tick: u64,
        body_id: u32,
        region: u8,
        level: u8,
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        mass: f64,
    },
    TotalsState {
        tick: u64,
        fine_count: u64,
        coarse_count: u64,
        mass: f64,
        px: f64,
        py: f64,
        energy: f64,
    },
    RegionCollapsed {
        tick: u64,
        region_x: u32,
        region_y: u32,
        body_count: u64,
        mass: f64,
        com_x: f64,
        com_y: f64,
        px: f64,
        py: f64,
        energy: f64,
    },
    RegionMultipole {
        tick: u64,
        region_x: u32,
        region_y: u32,
        mx: f64,
        my: f64,
        qxx: f64,
        qxy: f64,
        qyy: f64,
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

    pub fn new_gravity(
        mut out: W,
        world_w: u32,
        world_h: u32,
        body_count: u32,
    ) -> io::Result<Self> {
        out.write_all(MAGIC)?;
        out.write_all(&FORMAT_VERSION_GRAVITY.to_le_bytes())?;
        out.write_all(&world_w.to_le_bytes())?;
        out.write_all(&world_h.to_le_bytes())?;
        out.write_all(&body_count.to_le_bytes())?;
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
            Record::BodyState {
                tick,
                body_id,
                region,
                level,
                x,
                y,
                vx,
                vy,
                mass,
            } => {
                self.out.write_all(&[6u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&body_id.to_le_bytes())?;
                self.out.write_all(&[*region])?;
                self.out.write_all(&[*level])?;
                self.out.write_all(&x.to_le_bytes())?;
                self.out.write_all(&y.to_le_bytes())?;
                self.out.write_all(&vx.to_le_bytes())?;
                self.out.write_all(&vy.to_le_bytes())?;
                self.out.write_all(&mass.to_le_bytes())
            }
            Record::TotalsState {
                tick,
                fine_count,
                coarse_count,
                mass,
                px,
                py,
                energy,
            } => {
                self.out.write_all(&[7u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&fine_count.to_le_bytes())?;
                self.out.write_all(&coarse_count.to_le_bytes())?;
                self.out.write_all(&mass.to_le_bytes())?;
                self.out.write_all(&px.to_le_bytes())?;
                self.out.write_all(&py.to_le_bytes())?;
                self.out.write_all(&energy.to_le_bytes())
            }
            Record::RegionCollapsed {
                tick,
                region_x,
                region_y,
                body_count,
                mass,
                com_x,
                com_y,
                px,
                py,
                energy,
            } => {
                self.out.write_all(&[8u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&region_x.to_le_bytes())?;
                self.out.write_all(&region_y.to_le_bytes())?;
                self.out.write_all(&body_count.to_le_bytes())?;
                self.out.write_all(&mass.to_le_bytes())?;
                self.out.write_all(&com_x.to_le_bytes())?;
                self.out.write_all(&com_y.to_le_bytes())?;
                self.out.write_all(&px.to_le_bytes())?;
                self.out.write_all(&py.to_le_bytes())?;
                self.out.write_all(&energy.to_le_bytes())
            }
            Record::RegionMultipole {
                tick,
                region_x,
                region_y,
                mx,
                my,
                qxx,
                qxy,
                qyy,
            } => {
                self.out.write_all(&[9u8])?;
                self.out.write_all(&tick.to_le_bytes())?;
                self.out.write_all(&region_x.to_le_bytes())?;
                self.out.write_all(&region_y.to_le_bytes())?;
                self.out.write_all(&mx.to_le_bytes())?;
                self.out.write_all(&my.to_le_bytes())?;
                self.out.write_all(&qxx.to_le_bytes())?;
                self.out.write_all(&qxy.to_le_bytes())?;
                self.out.write_all(&qyy.to_le_bytes())
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
    body_count: Option<u32>,
    scratch: [u8; 72],
}

impl<R: Read> StreamReader<R> {
    pub fn new(mut input: R) -> Result<Self, ParseError> {
        let mut buf = [0u8; 16];
        read_exact(&mut input, &mut buf)?;
        if &buf[0..4] != MAGIC {
            return Err(ParseError::BadMagic);
        }
        let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        if version != FORMAT_VERSION && version != FORMAT_VERSION_GRAVITY {
            return Err(ParseError::BadVersion(version));
        }
        let world_w = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let world_h = u32::from_le_bytes(buf[12..16].try_into().unwrap());
        let mut body_count = None;
        if version == FORMAT_VERSION_GRAVITY {
            let mut bc = [0u8; 4];
            read_exact(&mut input, &mut bc)?;
            body_count = Some(u32::from_le_bytes(bc));
        }
        Ok(StreamReader {
            input,
            header: Some((world_w, world_h)),
            body_count,
            scratch: [0u8; 72],
        })
    }

    pub fn header(&self) -> (u32, u32) {
        self.header.expect("header consumed in new")
    }

    pub fn body_count(&self) -> Option<u32> {
        self.body_count
    }

    fn max_level(&self) -> u8 {
        if self.body_count.is_some() {
            2
        } else {
            1
        }
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
                if level > self.max_level() {
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
                if level > self.max_level() {
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
            6 => {
                self.take(54)?;
                let level = self.scratch[13];
                if level > self.max_level() {
                    return Err(ParseError::BadLevel(level));
                }
                Record::BodyState {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    body_id: u32::from_le_bytes(self.scratch[8..12].try_into().unwrap()),
                    region: self.scratch[12],
                    level,
                    x: f64::from_le_bytes(self.scratch[14..22].try_into().unwrap()),
                    y: f64::from_le_bytes(self.scratch[22..30].try_into().unwrap()),
                    vx: f64::from_le_bytes(self.scratch[30..38].try_into().unwrap()),
                    vy: f64::from_le_bytes(self.scratch[38..46].try_into().unwrap()),
                    mass: f64::from_le_bytes(self.scratch[46..54].try_into().unwrap()),
                }
            }
            7 => {
                self.take(56)?;
                Record::TotalsState {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    fine_count: u64::from_le_bytes(self.scratch[8..16].try_into().unwrap()),
                    coarse_count: u64::from_le_bytes(self.scratch[16..24].try_into().unwrap()),
                    mass: f64::from_le_bytes(self.scratch[24..32].try_into().unwrap()),
                    px: f64::from_le_bytes(self.scratch[32..40].try_into().unwrap()),
                    py: f64::from_le_bytes(self.scratch[40..48].try_into().unwrap()),
                    energy: f64::from_le_bytes(self.scratch[48..56].try_into().unwrap()),
                }
            }
            8 => {
                self.take(72)?;
                Record::RegionCollapsed {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    region_x: u32::from_le_bytes(self.scratch[8..12].try_into().unwrap()),
                    region_y: u32::from_le_bytes(self.scratch[12..16].try_into().unwrap()),
                    body_count: u64::from_le_bytes(self.scratch[16..24].try_into().unwrap()),
                    mass: f64::from_le_bytes(self.scratch[24..32].try_into().unwrap()),
                    com_x: f64::from_le_bytes(self.scratch[32..40].try_into().unwrap()),
                    com_y: f64::from_le_bytes(self.scratch[40..48].try_into().unwrap()),
                    px: f64::from_le_bytes(self.scratch[48..56].try_into().unwrap()),
                    py: f64::from_le_bytes(self.scratch[56..64].try_into().unwrap()),
                    energy: f64::from_le_bytes(self.scratch[64..72].try_into().unwrap()),
                }
            }
            9 => {
                self.take(56)?;
                Record::RegionMultipole {
                    tick: u64::from_le_bytes(self.scratch[0..8].try_into().unwrap()),
                    region_x: u32::from_le_bytes(self.scratch[8..12].try_into().unwrap()),
                    region_y: u32::from_le_bytes(self.scratch[12..16].try_into().unwrap()),
                    mx: f64::from_le_bytes(self.scratch[16..24].try_into().unwrap()),
                    my: f64::from_le_bytes(self.scratch[24..32].try_into().unwrap()),
                    qxx: f64::from_le_bytes(self.scratch[32..40].try_into().unwrap()),
                    qxy: f64::from_le_bytes(self.scratch[40..48].try_into().unwrap()),
                    qyy: f64::from_le_bytes(self.scratch[48..56].try_into().unwrap()),
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

    #[test]
    fn region_collapsed_roundtrip_gravity() {
        let mut buf = Vec::new();
        let rec = Record::RegionCollapsed {
            tick: 30,
            region_x: 1,
            region_y: 1,
            body_count: 3,
            mass: 4.25,
            com_x: 80.5,
            com_y: 81.5,
            px: 0.125,
            py: -0.25,
            energy: -1.5,
        };
        {
            let mut w = StreamWriter::new_gravity(&mut buf, 128, 128, 10).unwrap();
            w.write(&rec).unwrap();
            w.flush().unwrap();
        }
        assert_eq!(&buf[0..4], MAGIC);
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()), 2);
        assert_eq!(buf[20], 8);
        assert_eq!(buf.len(), 21 + 72);
        let mut r = StreamReader::new(&buf[..]).unwrap();
        assert_eq!(r.body_count(), Some(10));
        assert_eq!(r.next_record().unwrap(), Some(rec));
        assert_eq!(r.next_record().unwrap(), None);
    }

    #[test]
    fn gravity_stream_accepts_level_two() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new_gravity(&mut buf, 128, 128, 4).unwrap();
            w.write(&Record::RegionLevel {
                region_x: 1,
                region_y: 0,
                level: 2,
            })
            .unwrap();
            w.write(&Record::RegionState {
                tick: 1,
                region_x: 1,
                region_y: 0,
                level: 2,
                population: 2,
                hash: 7,
            })
            .unwrap();
            w.write(&Record::BodyState {
                tick: 1,
                body_id: 0,
                region: 2,
                level: 2,
                x: 1.0,
                y: 2.0,
                vx: 3.0,
                vy: 4.0,
                mass: 5.0,
            })
            .unwrap();
            w.flush().unwrap();
        }
        let mut r = StreamReader::new(&buf[..]).unwrap();
        assert_eq!(
            r.next_record().unwrap(),
            Some(Record::RegionLevel {
                region_x: 1,
                region_y: 0,
                level: 2,
            })
        );
        assert_eq!(
            r.next_record().unwrap(),
            Some(Record::RegionState {
                tick: 1,
                region_x: 1,
                region_y: 0,
                level: 2,
                population: 2,
                hash: 7,
            })
        );
        assert_eq!(
            r.next_record().unwrap(),
            Some(Record::BodyState {
                tick: 1,
                body_id: 0,
                region: 2,
                level: 2,
                x: 1.0,
                y: 2.0,
                vx: 3.0,
                vy: 4.0,
                mass: 5.0,
            })
        );
    }

    #[test]
    fn gravity_stream_rejects_level_three() {
        let mut buf = Vec::new();
        {
            let mut w = StreamWriter::new_gravity(&mut buf, 128, 128, 4).unwrap();
            w.write(&Record::RegionLevel {
                region_x: 0,
                region_y: 0,
                level: 3,
            })
            .unwrap();
        }
        assert_eq!(
            StreamReader::new(&buf[..])
                .unwrap()
                .next_record()
                .unwrap_err(),
            ParseError::BadLevel(3)
        );
    }

    #[test]
    fn region_multipole_roundtrip_gravity() {
        let mut buf = Vec::new();
        let rec = Record::RegionMultipole {
            tick: 30,
            region_x: 1,
            region_y: 1,
            mx: 512.5,
            my: -64.25,
            qxx: 1.5e3,
            qxy: -2.25,
            qyy: 9.75e2,
        };
        {
            let mut w = StreamWriter::new_gravity(&mut buf, 128, 128, 10).unwrap();
            w.write(&rec).unwrap();
            w.flush().unwrap();
        }
        assert_eq!(buf[20], 9);
        assert_eq!(buf.len(), 21 + 56);
        let mut r = StreamReader::new(&buf[..]).unwrap();
        assert_eq!(r.body_count(), Some(10));
        assert_eq!(r.next_record().unwrap(), Some(rec));
        assert_eq!(r.next_record().unwrap(), None);
    }
}

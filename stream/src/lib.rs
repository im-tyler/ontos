use std::io::{self, Write};

pub const MAGIC: &[u8; 4] = b"ONTO";
pub const FORMAT_VERSION: u32 = 1;

pub enum Record {
    Header { world_w: u32, world_h: u32 },
    TickHeader { tick: u64 },
    Snapshot { population: u64 },
    CellFlipped { tick: u64, x: u32, y: u32 },
    RegionLevel { region_x: u32, region_y: u32, level: u8 },
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
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
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
            w.write(&Record::CellFlipped { tick: 1, x: 3, y: 4 }).unwrap();
            w.write(&Record::RegionLevel { region_x: 1, region_y: 0, level: 0 }).unwrap();
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
}

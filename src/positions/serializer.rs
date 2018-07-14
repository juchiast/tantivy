use std::io;
use bitpacking::{BitPacker4x, BitPacker};
use positions::{COMPRESSION_BLOCK_SIZE, LONG_SKIP_INTERVAL};
use common::BinarySerializable;

lazy_static! {
    static ref BIT_PACKER: BitPacker4x = BitPacker4x::new();
}

pub struct PositionSerializer<W: io::Write> {
    write_stream: W,
    write_skiplist: W,
    block: Vec<u32>,
    buffer: Vec<u8>,
    num_ints: u64,
    long_skips: Vec<u64>,
    cumulated_num_bits: u64,
}

impl<W: io::Write> PositionSerializer<W> {
    pub fn new(write_stream: W, write_skiplist: W) -> PositionSerializer<W> {
        PositionSerializer {
            write_stream,
            write_skiplist,
            block: Vec::with_capacity(128),
            buffer: vec![0u8; 128 * 4],
            num_ints: 0u64,
            long_skips: Vec::new(),
            cumulated_num_bits: 0u64,
        }
    }

    pub fn positions_idx(&self) -> u64 {
        self.num_ints
    }

    pub fn write(&mut self, val: u32) -> io::Result<()> {
        self.block.push(val);
        self.num_ints += 1;
        if self.block.len() == COMPRESSION_BLOCK_SIZE {
            self.flush_block()?;
        }
        Ok(())
    }

    pub fn write_all(&mut self, vals: &[u32]) -> io::Result<()> {
        // TODO optimize
        for &val in vals {
            self.write(val)?;
        }
        Ok(())
    }

    fn flush_block(&mut self) -> io::Result<()> {
        let num_bits = BIT_PACKER.num_bits(&self.block[..]);
        self.cumulated_num_bits += num_bits as u64;
        self.write_skiplist.write(&[num_bits])?;
        let written_len = BIT_PACKER.compress(&self.block[..], &mut self.buffer, num_bits);
        self.write_stream.write_all(&self.buffer[..written_len])?;
        self.block.clear();
        if (self.num_ints % LONG_SKIP_INTERVAL) == 0u64 {
            self.long_skips.push(self.cumulated_num_bits);
        }
        Ok(())
    }

    pub fn close(mut self) -> io::Result<()> {
        if !self.block.is_empty() {
            self.block.resize(COMPRESSION_BLOCK_SIZE, 0u32);
            self.flush_block()?;
        }
        for &long_skip in &self.long_skips {
            long_skip.serialize(&mut self.write_skiplist)?;
        }
        (self.long_skips.len() as u32).serialize(&mut self.write_skiplist)?;
        self.write_skiplist.flush()?;
        self.write_stream.flush()?;
        Ok(())
    }
}
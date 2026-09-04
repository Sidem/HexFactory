use super::{ChunkSnapshot, Writer};

pub(super) fn write(writer: &mut Writer, chunks: &[ChunkSnapshot]) {
    writer.uvarint(chunks.len() as u64);
    // Generation order is stable but not coordinate-sorted, so these positions remain absolute.
    for chunk in chunks {
        writer.svarint(i64::from(chunk.chunk_q));
        writer.svarint(i64::from(chunk.chunk_r));
        writer.uvarint(chunk.entity_count as u64);
        writer.svarint(i64::from(chunk.x));
        writer.svarint(i64::from(chunk.y));
        writer.svarint(i64::from(chunk.span));
    }
}

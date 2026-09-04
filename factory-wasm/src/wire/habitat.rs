use super::{Cell, HabitatSnapshot, Writer};

pub(super) fn write(writer: &mut Writer, habitats: &[HabitatSnapshot]) {
    writer.uvarint(habitats.len() as u64);
    let mut previous = Cell::default();
    for habitat in habitats {
        previous.write_delta(writer, habitat.q, habitat.r, habitat.x, habitat.y);
        writer.uvarint(u64::from(habitat.radius));
        writer.uvarint(u64::from(habitat.capacity));
        writer.u8(habitat.discharge);
    }
}

#[cfg(test)]
pub(super) fn read(reader: &mut super::decode::Reader<'_>) -> super::HabitatsDelta {
    let replace = reader.u8() & super::PATCH_REPLACE != 0;
    let mut q = 0;
    let mut r = 0;
    let mut x = 0;
    let mut y = 0;
    let changed = (0..reader.count())
        .map(|_| {
            q += reader.svarint() as i32;
            r += reader.svarint() as i32;
            x += reader.svarint() as i32;
            y += reader.svarint() as i32;
            HabitatSnapshot {
                q,
                r,
                x,
                y,
                radius: reader.uvarint() as u32,
                capacity: reader.uvarint() as u16,
                discharge: reader.u8(),
            }
        })
        .collect();
    super::HabitatsDelta { replace, changed }
}

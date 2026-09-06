use super::Writer;

pub(super) fn write(writer: &mut Writer, herds: &super::HerdsDelta) {
    writer.bool(herds.replace);
    writer.uvarint(herds.changed.len() as u64);
    for h in &herds.changed {
        writer.uvarint(u64::from(h.id));
        writer.uvarint(u64::from(h.species));
        for v in [h.from.0, h.from.1, h.to.0, h.to.1] {
            writer.svarint(i64::from(v));
        }
        writer.uvarint(h.left_tick);
        writer.uvarint(h.arrive_tick);
        for v in [
            h.count,
            h.drive as u16,
            h.hunger,
            h.thirst,
            h.alarm,
            h.healthy_ticks,
            h.shortage_ticks,
        ] {
            writer.uvarint(u64::from(v));
        }
        writer.uvarint(h.hunt_tick);
    }
    writer.uvarint(herds.removed.len() as u64);
    for &id in &herds.removed {
        writer.uvarint(u64::from(id));
    }
}

#[cfg(test)]
pub(super) fn read(reader: &mut super::decode::Reader<'_>) -> super::HerdsDelta {
    use super::{Drive, Herd};
    let replace = reader.bool();
    let changed = (0..reader.uvarint())
        .map(|_| {
            let id = reader.uvarint() as u32;
            let species = reader.uvarint() as u16;
            let from = (reader.svarint() as i32, reader.svarint() as i32);
            let to = (reader.svarint() as i32, reader.svarint() as i32);
            let left_tick = reader.uvarint();
            let arrive_tick = reader.uvarint();
            let count = reader.uvarint() as u16;
            let drive = match reader.uvarint() {
                0 => Drive::Rest,
                1 => Drive::Graze,
                2 => Drive::Thirst,
                3 => Drive::Flee,
                _ => panic!("drive"),
            };
            Herd {
                id,
                species,
                from,
                to,
                left_tick,
                arrive_tick,
                count,
                drive,
                hunger: reader.uvarint() as u16,
                thirst: reader.uvarint() as u16,
                alarm: reader.uvarint() as u16,
                healthy_ticks: reader.uvarint() as u16,
                shortage_ticks: reader.uvarint() as u16,
                hunt_tick: reader.uvarint(),
            }
        })
        .collect();
    let removed = (0..reader.uvarint())
        .map(|_| reader.uvarint() as u32)
        .collect();
    super::HerdsDelta {
        replace,
        changed,
        removed,
    }
}

use aviutl2::anyhow::{Context, bail};

#[derive(Debug, Clone, PartialEq)]
pub struct TempoEvent {
    pub starting_time: f64,
    pub bpm: f64,
    pub beat: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MidiTempoMap {
    pub tempos: Vec<TempoEvent>,
    pub last_event_time: f64,
}

#[derive(Debug, Clone, Copy)]
struct MidiMetaAtTick {
    tick: u64,
    tempo: Option<f64>,
    beat: Option<u32>,
}

pub fn load_midi(path: &std::path::Path) -> aviutl2::common::AnyResult<MidiTempoMap> {
    let bytes = std::fs::read(path).with_context(|| format!("MIDIを読み込めません: {path:?}"))?;
    parse_midi(&bytes)
}

fn parse_midi(bytes: &[u8]) -> aviutl2::common::AnyResult<MidiTempoMap> {
    let smf = midly::Smf::parse(bytes).context("MIDIの解析に失敗しました")?;
    let ticks_per_quarter = match smf.header.timing {
        midly::Timing::Metrical(ticks) => ticks.as_int() as u64,
        midly::Timing::Timecode(_, _) => bail!("SMPTE timecode の MIDI は未対応です"),
    };

    let mut metas = Vec::new();
    let mut last_tick = 0;
    for track in &smf.tracks {
        let mut tick = 0_u64;
        for event in track {
            tick = tick.saturating_add(event.delta.as_int() as u64);
            last_tick = last_tick.max(tick);
            match event.kind {
                midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(tempo)) => {
                    let microseconds_per_quarter = tempo.as_int() as f64;
                    metas.push(MidiMetaAtTick {
                        tick,
                        tempo: Some(60_000_000.0 / microseconds_per_quarter),
                        beat: None,
                    });
                }
                midly::TrackEventKind::Meta(midly::MetaMessage::TimeSignature(
                    numerator,
                    _denominator,
                    _ticks_per_click,
                    _thirtyseconds_per_quarter,
                )) => {
                    metas.push(MidiMetaAtTick {
                        tick,
                        tempo: None,
                        beat: Some(numerator as u32),
                    });
                }
                _ => {}
            }
        }
    }

    metas.sort_by_key(|meta| meta.tick);

    let mut tempos = vec![TempoEvent {
        starting_time: 0.0,
        bpm: 120.0,
        beat: 4,
    }];
    let mut current_bpm = 120.0;
    let mut current_beat = 4;
    let mut current_time = 0.0;
    let mut previous_tick = 0_u64;
    let mut index = 0;

    while index < metas.len() {
        let tick = metas[index].tick;
        current_time += ticks_to_seconds(tick - previous_tick, ticks_per_quarter, current_bpm);
        previous_tick = tick;

        let mut changed = false;
        while index < metas.len() && metas[index].tick == tick {
            if let Some(bpm) = metas[index].tempo
                && (current_bpm - bpm).abs() > f64::EPSILON
            {
                current_bpm = bpm;
                changed = true;
            }
            if let Some(beat) = metas[index].beat
                && current_beat != beat
            {
                current_beat = beat;
                changed = true;
            }
            index += 1;
        }

        if changed {
            let tempo = TempoEvent {
                starting_time: current_time,
                bpm: current_bpm,
                beat: current_beat,
            };
            if tick == 0 {
                tempos[0] = tempo;
            } else {
                tempos.push(tempo);
            }
        }
    }

    current_time += ticks_to_seconds(last_tick - previous_tick, ticks_per_quarter, current_bpm);

    Ok(MidiTempoMap {
        tempos,
        last_event_time: current_time,
    })
}

fn ticks_to_seconds(ticks: u64, ticks_per_quarter: u64, bpm: f64) -> f64 {
    ticks as f64 * 60.0 / (ticks_per_quarter as f64 * bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tempo_map_and_last_event_time() {
        let midi = [
            b"MThd".as_slice(),
            &[0, 0, 0, 6, 0, 1, 0, 2, 1, 224],
            b"MTrk".as_slice(),
            &[0, 0, 0, 28],
            &[
                0x00, 0xff, 0x58, 0x04, 0x03, 0x02, 0x18, 0x08, 0x00, 0xff, 0x51, 0x03, 0x07, 0xa1,
                0x20, 0x83, 0x60, 0xff, 0x51, 0x03, 0x03, 0xd0, 0x90, 0x83, 0x60, 0xff, 0x2f, 0x00,
            ],
            b"MTrk".as_slice(),
            &[0, 0, 0, 13],
            &[
                0x00, 0x90, 0x3c, 0x40, 0x87, 0x40, 0x80, 0x3c, 0x00, 0x00, 0xff, 0x2f, 0x00,
            ],
        ]
        .concat();

        let map = parse_midi(&midi).unwrap();

        assert_eq!(map.tempos.len(), 2);
        assert_eq!(
            map.tempos[0],
            TempoEvent {
                starting_time: 0.0,
                bpm: 120.0,
                beat: 3,
            }
        );
        assert_eq!(map.tempos[1].bpm, 240.0);
        assert_eq!(map.tempos[1].beat, 3);
        assert!((map.tempos[1].starting_time - 0.5).abs() < 0.000_001);
        assert!((map.last_event_time - 0.75).abs() < 0.000_001);
    }
}

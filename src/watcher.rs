enum WatcherMessage {
    FrameMoved,
    ObjectUpdated,
    SceneChanged,
    SetGridMultiplier(i32),
    Stop,
}

pub struct WatcherThread {
    thread: Option<std::thread::JoinHandle<()>>,
    sender: std::sync::mpsc::Sender<WatcherMessage>,
}

pub struct WatcherState {
    pub multiplier: i32,
}

impl WatcherThread {
    pub fn start() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut state = WatcherState::new();
            loop {
                if !crate::EDIT_HANDLE.is_ready() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                match receiver.recv() {
                    Ok(
                        WatcherMessage::FrameMoved
                        | WatcherMessage::ObjectUpdated
                        | WatcherMessage::SceneChanged,
                    ) => {
                        state.update_if_changed();
                    }
                    Ok(WatcherMessage::SetGridMultiplier(multiplier)) => {
                        state.multiplier = multiplier;
                        state.update_if_changed();
                    }
                    Ok(WatcherMessage::Stop) => break,
                    Err(e) => {
                        tracing::error!("Watcher thread receiver error: {e}");
                        break;
                    }
                }
            }
        });
        Self {
            thread: Some(thread),
            sender,
        }
    }

    pub fn notify_frame_moved(&self) {
        let _ = self.sender.send(WatcherMessage::FrameMoved);
    }

    pub fn notify_object_updated(&self) {
        let _ = self.sender.send(WatcherMessage::ObjectUpdated);
    }

    pub fn notify_scene_changed(&self) {
        let _ = self.sender.send(WatcherMessage::SceneChanged);
    }

    pub fn set_grid_multiplier(&self, multiplier: i32) {
        let _ = self
            .sender
            .send(WatcherMessage::SetGridMultiplier(multiplier));
    }
}

#[derive(Debug, Clone, Copy)]
struct BpmGrid {
    bpm: f32,
    beat: usize,
    offset: f32,
}
impl BpmGrid {
    fn equal(&self, other: &Self) -> bool {
        (self.bpm - other.bpm).abs() < f32::EPSILON
            && self.beat == other.beat
            && (self.offset - other.offset).abs() < f32::EPSILON
    }

    fn with_multiplier(mut self, multiplier: i32) -> Self {
        if multiplier == 0 {
            return self;
        }

        let original_measure_length = 60.0 * self.beat as f32 / self.bpm;
        self.beat = ((self.beat as f32 * 2f32.powi(multiplier)).round() as usize).max(1);
        self.bpm = 60.0 * self.beat as f32 / original_measure_length;
        self
    }
}

impl WatcherState {
    fn new() -> Self {
        Self { multiplier: 0 }
    }
    fn update_if_changed(&self) {
        if let Err(e) = self.update_if_changed_impl() {
            tracing::error!("Failed to update grid: {e}");
        }
    }
    fn update_if_changed_impl(&self) -> anyhow::Result<()> {
        if crate::EDIT_HANDLE.get_edit_state()? != aviutl2::generic::EditState::Edit {
            return Ok(());
        }

        let info = crate::EDIT_HANDLE.get_edit_info();
        let target_grid = crate::EDIT_HANDLE
            .call_read_section(|read| {
                let overlapping_bpm_object = (0..info.layer).rev().find_map(|layer| {
                    let maybe_overlap = read.find_object_after(layer, info.frame).ok()??;
                    let range = read.get_object_layer_frame(maybe_overlap).ok()?;
                    if range.start <= info.frame
                        && info.frame <= range.end
                        && read
                            .count_object_effect(maybe_overlap, crate::filter::OBJECT_NAME)
                            .ok()?
                            > 0
                    {
                        let alias: aviutl2::alias::Table =
                            read.get_object_alias(maybe_overlap).ok()?.parse().ok()?;
                        let disabled = alias
                            .get_table("Object.0")?
                            .get_value("effect.disable")
                            .is_some_and(|v| v == "1");
                        if disabled {
                            return None;
                        }
                        Some(maybe_overlap)
                    } else {
                        None
                    }
                });

                let Some(overlap) = overlapping_bpm_object else {
                    return Ok(None);
                };
                let overlap = read.object(overlap);

                let bpm: f32 = overlap
                    .get_effect_item(crate::filter::OBJECT_NAME, 0, "BPM")?
                    .parse()?;
                let beat: usize = overlap
                    .get_effect_item(crate::filter::OBJECT_NAME, 0, "拍子")?
                    .parse()?;
                let offset: f32 = overlap
                    .get_effect_item(crate::filter::OBJECT_NAME, 0, "オフセット")?
                    .parse()?;
                let starting_frame = overlap.get_layer_frame()?.start;
                let offset = starting_frame as f32 * *info.fps.denom() as f32
                    / *info.fps.numer() as f32
                    + offset;
                Ok(Some(BpmGrid { bpm, beat, offset }))
            })
            .map_err(anyhow::Error::from)
            .flatten()?;

        let Some(mut target_grid) = target_grid else {
            return Ok(());
        };

        target_grid = target_grid.with_multiplier(self.multiplier);

        let current_grid = BpmGrid {
            bpm: info.grid_bpm_tempo,
            beat: info.grid_bpm_beat,
            offset: info.grid_bpm_offset,
        };

        if !current_grid.equal(&target_grid) {
            tracing::info!("Updating grid: {current_grid:?} -> {target_grid:?}");
            crate::EDIT_HANDLE.call_edit_section(|edit| {
                edit.set_grid_bpm(target_grid.bpm, target_grid.beat, target_grid.offset)
            })??;
        }

        Ok(())
    }
}

impl Drop for WatcherThread {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.sender.send(WatcherMessage::Stop);
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplier_keeps_measure_length_when_increasing() {
        let grid = BpmGrid {
            bpm: 120.0,
            beat: 4,
            offset: 1.25,
        };

        let multiplied = grid.with_multiplier(1);

        assert_eq!(multiplied.beat, 8);
        assert_eq!(multiplied.bpm, 240.0);
        assert_eq!(multiplied.offset, grid.offset);
        assert_eq!(measure_length(multiplied), measure_length(grid));
    }

    #[test]
    fn multiplier_keeps_measure_length_when_decreasing() {
        let grid = BpmGrid {
            bpm: 120.0,
            beat: 4,
            offset: 1.25,
        };

        let multiplied = grid.with_multiplier(-1);

        assert_eq!(multiplied.beat, 2);
        assert_eq!(multiplied.bpm, 60.0);
        assert_eq!(multiplied.offset, grid.offset);
        assert_eq!(measure_length(multiplied), measure_length(grid));
    }

    fn measure_length(grid: BpmGrid) -> f32 {
        60.0 * grid.beat as f32 / grid.bpm
    }
}

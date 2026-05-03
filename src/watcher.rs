enum WatcherMessage {
    Stop,
}

pub struct WatcherThread {
    thread: Option<std::thread::JoinHandle<()>>,
    sender: std::sync::mpsc::Sender<WatcherMessage>,
}

impl WatcherThread {
    pub fn start() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            loop {
                if let Ok(WatcherMessage::Stop) = receiver.try_recv() {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(100));

                if !crate::EDIT_HANDLE.is_ready() {
                    continue;
                }

                match Self::check() {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("check failed: {:#}", e);
                    }
                }
            }
        });
        Self {
            thread: Some(thread),
            sender,
        }
    }

    fn check() -> anyhow::Result<()> {
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
                let starting_frame = overlap.get_layer_frame()?.start;
                let offset =
                    starting_frame as f32 * *info.fps.denom() as f32 / *info.fps.numer() as f32;
                Ok(Some((bpm, beat, offset)))
            })
            .map_err(anyhow::Error::from)
            .flatten()?;

        let Some(target_grid) = target_grid else {
            return Ok(());
        };

        let current_grid = (
            info.grid_bpm_tempo,
            info.grid_bpm_beat,
            info.grid_bpm_offset,
        );

        if (current_grid.0 - target_grid.0).abs() > f32::EPSILON
            || current_grid.1 != target_grid.1
            || (current_grid.2 - target_grid.2).abs() > f32::EPSILON
        {
            tracing::info!(
                "Updating grid: bpm={} beat={} offset={}",
                target_grid.0,
                target_grid.1,
                target_grid.2
            );
            crate::EDIT_HANDLE.call_edit_section(|edit| {
                edit.set_grid_bpm(target_grid.0 as _, target_grid.1 as _, target_grid.2 as _)
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

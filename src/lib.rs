mod filter;
mod midi;
mod watcher;

pub static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle =
    aviutl2::generic::GlobalEditHandle::new();

#[aviutl2::plugin(GenericPlugin)]
struct BpmObjectAux2 {
    object: aviutl2::generic::SubPlugin<crate::filter::BpmObject>,
    _watcher_thread: crate::watcher::WatcherThread,
}

impl aviutl2::generic::GenericPlugin for BpmObjectAux2 {
    fn new(info: aviutl2::common::AviUtl2Info) -> aviutl2::common::AnyResult<Self> {
        aviutl2::tracing_subscriber::fmt()
            .with_max_level(if cfg!(debug_assertions) {
                tracing::Level::DEBUG
            } else {
                tracing::Level::INFO
            })
            .event_format(aviutl2::logger::AviUtl2Formatter)
            .with_writer(aviutl2::logger::AviUtl2LogWriter)
            .init();
        Ok(Self {
            object: aviutl2::generic::SubPlugin::new_filter_plugin(&info)?,
            _watcher_thread: crate::watcher::WatcherThread::start(),
        })
    }

    fn plugin_info(&self) -> aviutl2::generic::GenericPluginTable {
        aviutl2::generic::GenericPluginTable {
            name: "bpm_object.aux2".to_string(),
            information: "".to_string(),
        }
    }

    fn register(&mut self, registry: &mut aviutl2::generic::HostAppHandle) {
        registry.register_filter_plugin(&self.object);
        registry.register_menus::<Self>();
        EDIT_HANDLE.init(registry.create_edit_handle());
    }
}

#[aviutl2::generic::menus]
impl BpmObjectAux2 {
    #[import(name = "[bpm_object.aux2] MIDIからテンポを適用")]
    fn import_midi(&mut self) -> aviutl2::common::AnyResult<()> {
        let file = native_dialog::FileDialogBuilder::default()
            .add_filter("MIDI file", ["mid", "midi"])
            .set_owner(&unsafe { EDIT_HANDLE.get_host_app_window().unwrap() })
            .open_single_file()
            .show()?;
        let Some(file) = file else {
            return Ok(());
        };

        let tempo_map = crate::midi::load_midi(&file)?;
        aviutl2::tracing::info!(
            "Loaded MIDI tempo map: tempos={:?}, last_event_time={}",
            tempo_map.tempos,
            tempo_map.last_event_time
        );

        EDIT_HANDLE.call_edit_section(|edit| {
            for i in (0..=edit.info.layer_max).rev() {
                for (position, object) in edit.objects_in_layer(i) {
                    edit.move_object(object, i + 1, position.start)?;
                }
            }

            for (i, tempo_event) in tempo_map.tempos.iter().enumerate() {
                let frame = (tempo_event.starting_time * *edit.info.fps.numer() as f64
                    / *edit.info.fps.denom() as f64) as usize;
                let ending_frame = (if let Some(next) = tempo_map.tempos.get(i + 1) {
                    next.starting_time * *edit.info.fps.numer() as f64
                        / *edit.info.fps.denom() as f64
                } else {
                    tempo_map.last_event_time * *edit.info.fps.numer() as f64
                        / *edit.info.fps.denom() as f64
                }) as usize;
                let object = edit.create_object(
                    crate::filter::OBJECT_NAME,
                    0,
                    frame,
                    Some(ending_frame - frame),
                )?;
                edit.set_object_effect_item(
                    object,
                    crate::filter::OBJECT_NAME,
                    0,
                    "BPM",
                    &tempo_event.bpm.to_string(),
                )?;
                edit.set_object_effect_item(
                    object,
                    crate::filter::OBJECT_NAME,
                    0,
                    "拍子",
                    &tempo_event.beat.to_string(),
                )?;
            }

            anyhow::Ok(())
        })??;

        Ok(())
    }
}

aviutl2::register_generic_plugin!(BpmObjectAux2);

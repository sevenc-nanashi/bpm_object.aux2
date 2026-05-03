mod filter;
mod watcher;
use aviutl2::{anyhow::Context, filter::FilterConfigItems, tracing};

pub static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle =
    aviutl2::generic::GlobalEditHandle::new();

#[aviutl2::plugin(GenericPlugin)]
struct BpmObjectAux2 {
    object: aviutl2::generic::SubPlugin<crate::filter::BpmObject>,
    watcher_thread: crate::watcher::WatcherThread,
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
            watcher_thread: crate::watcher::WatcherThread::start(),
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
        EDIT_HANDLE.init(registry.create_edit_handle());
    }
}

aviutl2::register_generic_plugin!(BpmObjectAux2);

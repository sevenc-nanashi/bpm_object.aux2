use aviutl2::filter::FilterConfigItems;

pub const OBJECT_NAME: &str = "BPMオブジェクト";

#[aviutl2::plugin(FilterPlugin)]
pub struct BpmObject;

#[aviutl2::filter::filter_config_items]
struct BpmObjectAuf2Config {
    #[track(name = "BPM", default = 120.0, range=0.0..=640.0, step = 0.001)]
    _tempo: f64,
    #[track(name = "拍子", default = 4, range=1..=16, step = 1)]
    _beat: u32,
    #[track(name = "オフセット", default = 0.0, range=-10.0..=10.0, step = 0.001)]
    _offset: f64,
}

impl aviutl2::filter::FilterPlugin for BpmObject {
    fn new(_info: aviutl2::common::AviUtl2Info) -> aviutl2::common::AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> aviutl2::filter::FilterPluginTable {
        aviutl2::filter::FilterPluginTable {
            name: OBJECT_NAME.to_string(),
            label: None,
            information: "BPMオブジェクト".to_string(),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags {
                video: false,
                audio: true,
                input: true,
            }),
            config_items: BpmObjectAuf2Config::to_config_items(),
        }
    }

    fn proc_audio(
        &self,
        _config: &[aviutl2::filter::FilterConfigItem],
        _audio: &mut aviutl2::filter::FilterProcAudio,
    ) -> aviutl2::common::AnyResult<()> {
        Ok(())
    }
}

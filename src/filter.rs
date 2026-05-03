use aviutl2::{anyhow::Context, filter::FilterConfigItems, tracing};

pub const OBJECT_NAME: &str = "BPMオブジェクト";

#[aviutl2::plugin(FilterPlugin)]
pub struct BpmObject;

#[aviutl2::filter::filter_config_items]
struct BpmObjectAuf2Config {
    #[track(name = "BPM", default = 120.0, range=0.0..=640.0, step = 0.1)]
    bpm: f64,
    #[track(name = "拍子", default = 4, range=1..=16, step = 1)]
    beat: u32,
    #[button(name = "設定")]
    set_bpm: fn(),
}

fn set_bpm(edit: &mut aviutl2::generic::EditSection) -> aviutl2::AnyResult<()> {
    let object = edit.object(
        edit.get_focused_object()?
            .context("オブジェクトが選択されていません")?,
    );
    let bpm: f64 = object
        .get_effect_item(OBJECT_NAME, 0, "BPM")?
        .parse()?;
    let beat: u32 = object
        .get_effect_item(OBJECT_NAME, 0, "拍子")?
        .parse()?;

    let starting_frame = object.get_layer_frame()?.start;
    let offset =
        starting_frame as f64 * *edit.info.fps.denom() as f64 / *edit.info.fps.numer() as f64;
    tracing::info!(
        "BPMオブジェクトを設定: bpm={}, beat={}, offset={}",
        bpm,
        beat,
        offset
    );

    edit.set_grid_bpm(bpm as _, beat as _, offset as _)?;

    Ok(())
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
                as_object: true,
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

use converter_core::model::RunControl;
use converter_download::TransferControl;
#[derive(Clone, Default)]
pub struct Controls {
    pub download: TransferControl,
    pub conversion: RunControl,
}

use crate::unity::{ReleaseStream, Version};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseDataPayload {
    #[serde(rename = "offset")]
    pub offset: usize,
    #[serde(rename = "limit")]
    pub limit: usize,
    #[serde(rename = "total")]
    pub total: usize,
    #[serde(rename = "results")]
    pub results: Vec<ReleaseData>,
}

#[allow(dead_code)]
#[derive(Deserialize, Serialize, Debug)]
pub struct ReleaseData {
    #[serde(rename = "version")]
    pub version: Version,
    #[serde(rename = "releaseDate")]
    pub release_date: DateTime<Utc>,
    #[serde(rename = "stream")]
    pub stream: ReleaseStream,
    #[serde(rename = "releaseNotes")]
    pub release_notes: ReleaseNotes,
    #[serde(rename = "shortRevision")]
    pub short_revision: String,
    #[serde(rename = "skuFamily")]
    pub sku_family: String,
    #[serde(rename = "unityHubDeepLink")]
    pub unity_hub_deep_link: String,

    // Not used
    #[serde(skip, rename = "recommended")]
    pub recommended: bool,
    #[serde(skip, rename = "downloads")]
    pub downloads: Option<Vec<DownloadsElement>>,
    #[serde(skip, rename = "thirdPartyNotices")]
    pub third_party_notices: Option<Vec<ThirdPartyNoticesElement>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadsElement {
    #[serde(rename = "architecture")]
    pub architecture: String,
    #[serde(rename = "downloadSize")]
    pub download_size: SizeUnitValue,
    #[serde(rename = "installedSize")]
    pub installed_size: SizeUnitValue,
    #[serde(rename = "integrity")]
    pub integrity: Option<String>,
    #[serde(rename = "modules")]
    pub modules: Vec<ModulesElement>,
    #[serde(rename = "platform")]
    pub platform: String,
    #[serde(rename = "type")]
    pub download_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SizeUnitValue {
    #[serde(rename = "unit")]
    pub unit: String,
    #[serde(rename = "value")]
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModulesElement {
    #[serde(rename = "category")]
    pub category: String,
    #[serde(rename = "description")]
    pub description: String,
    #[serde(rename = "downloadSize")]
    pub download_size: SizeUnitValue,
    #[serde(rename = "eula")]
    pub eula: Option<Vec<EulaElement>>,
    #[serde(rename = "hidden")]
    pub hidden: bool,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "installedSize")]
    pub installed_size: SizeUnitValue,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "preSelected")]
    pub pre_selected: bool,
    #[serde(rename = "required")]
    pub required: bool,
    #[serde(rename = "slug")]
    pub slug: String,
    #[serde(rename = "subModules")]
    pub sub_modules: Vec<SubModulesElement>,
    #[serde(rename = "type")]
    pub module_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
    #[serde(rename = "destination")]
    pub destination: Option<String>,
    #[serde(rename = "integrity")]
    pub integrity: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EulaElement {
    #[serde(rename = "label")]
    pub label: String,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "type")]
    pub eula_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubModulesElement {
    #[serde(rename = "category")]
    pub category: String,
    #[serde(rename = "description")]
    pub description: String,
    #[serde(rename = "destination")]
    pub destination: Option<String>,
    #[serde(rename = "downloadSize")]
    pub download_size: SizeUnitValue,
    #[serde(rename = "eula")]
    pub eula: Option<Vec<EulaElement>>,
    #[serde(rename = "extractedPathRename")]
    pub extracted_path_rename: Option<ExtractedPathRename>,
    #[serde(rename = "hidden")]
    pub hidden: bool,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "installedSize")]
    pub installed_size: SizeUnitValue,
    #[serde(rename = "integrity")]
    pub integrity: Option<String>,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "preSelected")]
    pub pre_selected: bool,
    #[serde(rename = "required")]
    pub required: bool,
    #[serde(rename = "slug")]
    pub slug: String,
    #[serde(rename = "subModules")]
    pub sub_modules: Vec<SubModulesElement>,
    #[serde(rename = "type")]
    pub module_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedPathRename {
    #[serde(rename = "from")]
    pub from: String,
    #[serde(rename = "to")]
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseNotes {
    #[serde(rename = "type")]
    pub notes_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThirdPartyNoticesElement {
    #[serde(rename = "originalFileName")]
    pub original_file_name: String,
    #[serde(rename = "type")]
    pub notice_type: String, // Renamed due to keyword collision
    #[serde(rename = "url")]
    pub url: String,
}

use std::{cmp::Reverse, collections::HashMap, sync::Arc, time::Duration};

use axum::{
    body::StreamBody,
    extract::{Query, State},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    Json,
};
use futures::{
    stream::{self, Stream},
    FutureExt,
};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::{
    clip::{self, Clip, ClipOrder},
    config::{self, Config},
    error::AppError,
    ffmpeg::{self, find_stream_url},
    stash_api::{Api, Marker},
    AppState,
};

#[derive(Serialize, Debug)]
pub struct Tag {
    pub name: String,
    pub id: String,
    pub count: i64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Performer {
    pub name: String,
    pub id: String,
    pub scene_count: i64,
    pub image_url: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub image_url: String,
    pub performers: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarkerResult {
    pub dtos: Vec<Marker>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum FilterMode {
    Performers,
    Tags,
    Scenes,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarkerOptions {
    pub selected_ids: String,
    pub mode: FilterMode,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum Resolution {
    #[serde(rename = "720")]
    SevenTwenty,
    #[serde(rename = "1080")]
    TenEighty,
    #[serde(rename = "4K")]
    FourK,
}

impl Resolution {
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            Resolution::SevenTwenty => (1280, 720),
            Resolution::TenEighty => (1920, 1080),
            Resolution::FourK => (3840, 2160),
        }
    }
}

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectedMarker {
    pub id: String,
    pub duration: Option<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateVideoBody {
    pub select_mode: FilterMode,
    pub selected_ids: Vec<String>,
    pub clip_order: ClipOrder,
    pub clip_duration: u32,
    pub output_resolution: Resolution,
    pub output_fps: u32,
    pub selected_markers: Vec<SelectedMarker>,
    pub markers: Vec<Marker>,
    pub id: String,
    pub file_name: String,
    pub clips: Vec<Clip>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateClipsBody {
    pub clip_order: ClipOrder,
    pub clip_duration: u32,
    pub selected_markers: Vec<SelectedMarker>,
    pub markers: Vec<Marker>,
    pub select_mode: FilterMode,
}

pub fn add_api_key(url: &str, api_key: &str) -> String {
    let mut url = Url::parse(url).expect("invalid url");
    url.query_pairs_mut().append_pair("apikey", api_key);
    url.to_string()
}

#[axum::debug_handler]
pub async fn fetch_tags() -> Result<Json<Vec<Tag>>, AppError> {
    let api = Api::load_config().await?;
    let tags = api.find_tags().await?;
    let mut tags: Vec<_> = tags
        .into_iter()
        .map(|t| Tag {
            name: t.name,
            id: t.id,
            count: t.scene_marker_count.unwrap_or_default(),
        })
        .filter(|t| t.count > 0)
        .collect();
    tags.sort_by_key(|t| Reverse(t.count));

    tracing::debug!("returning tags {:?}", tags);

    Ok(Json(tags))
}

#[axum::debug_handler]
pub async fn fetch_performers() -> Result<Json<Vec<Performer>>, AppError> {
    let config = Config::get().await?;
    let api = Api::from_config(&config);
    let performers = api.find_performers().await?;
    let mut performers: Vec<_> = performers
        .into_iter()
        .map(|p| Performer {
            id: p.id,
            scene_count: p.scene_count.unwrap_or_default(),
            name: p.name,
            image_url: p.image_path.map(|url| add_api_key(&url, &config.api_key)),
        })
        .filter(|p| p.scene_count > 0)
        .collect();
    performers.sort_by_key(|t| Reverse(t.scene_count));

    tracing::debug!("returning performers {:?}", performers);

    Ok(Json(performers))
}

#[axum::debug_handler]
pub async fn fetch_markers(
    Query(query): Query<MarkerOptions>,
) -> Result<Json<MarkerResult>, AppError> {
    let config = Config::get().await?;
    let api = Api::from_config(&config);
    tracing::info!("fetching markers for query {query:?}");
    let ids: Vec<_> = query.selected_ids.split(',').map(From::from).collect();

    let markers = api.find_markers(ids, query.mode).await?;
    Ok(Json(MarkerResult { dtos: markers }))
}

#[axum::debug_handler]
pub async fn fetch_scenes() -> Result<Json<Vec<Scene>>, AppError> {
    let config = Config::get().await?;
    let api = Api::from_config(&config);
    let api_key = &config.api_key;
    let scenes = api.find_scenes().await?;
    let scenes = scenes
        .into_iter()
        .map(|s| Scene {
            id: s.id,
            title: s.title.unwrap_or_default(),
            performers: s.performers.into_iter().map(|p| p.name).collect(),
            image_url: s
                .paths
                .screenshot
                .map(|s| add_api_key(&s, api_key))
                .unwrap_or_default(),
        })
        .collect();
    Ok(Json(scenes))
}

async fn create_video_inner(
    state: State<Arc<AppState>>,
    mut body: CreateVideoBody,
) -> Result<(), AppError> {
    body.markers
        .retain(|e| body.selected_markers.iter().any(|m| m.id == e.id));
    let clips = state.ffmpeg.gather_clips(&body).await?;
    state.ffmpeg.compile_clips(&body, clips).await?;
    Ok(())
}

#[axum::debug_handler]
pub async fn create_video(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateVideoBody>,
) -> StatusCode {
    tracing::debug!("received json body: {:?}", body);
    tokio::spawn(async move {
        if let Err(e) = create_video_inner(state, body).await {
            tracing::error!("error: {e:?}");
        }
    });

    StatusCode::NO_CONTENT
}

#[axum::debug_handler]
pub async fn get_progress() -> Sse<impl Stream<Item = Result<Event, serde_json::Error>>> {
    let stream = futures::StreamExt::flat_map(stream::repeat_with(ffmpeg::get_progress), |f| {
        f.into_stream()
    });
    let stream = stream
        .map(|p| Event::default().json_data(p))
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilenameQuery {
    file_name: String,
}

#[axum::debug_handler]
pub async fn download_video(
    state: State<Arc<AppState>>,
    Query(FilenameQuery { file_name }): Query<FilenameQuery>,
) -> Result<impl IntoResponse, AppError> {
    use axum::{http::header, response::AppendHeaders};

    tracing::info!("downloading video {file_name}");
    let path = state.ffmpeg.video_dir.join(&file_name);
    let file = tokio::fs::File::open(path).await?;
    let stream = ReaderStream::new(file);
    let content_disposition = format!("attachment; filename=\"{}\"", file_name);

    let headers = AppendHeaders([
        (header::CONTENT_TYPE, "video/mp4".to_string()),
        (header::CONTENT_DISPOSITION, content_disposition),
    ]);

    let body = StreamBody::new(stream);
    Ok((headers, body))
}

#[axum::debug_handler]
pub async fn get_config() -> impl IntoResponse {
    match Config::get().await {
        Ok(config) => Json(Some(config)),
        Err(_) => Json(None),
    }
}

#[axum::debug_handler]
pub async fn set_config(Json(config): Json<Config>) -> Result<StatusCode, AppError> {
    tracing::info!("setting config with URL {}", config.stash_url);
    config::set_config(config).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct ClipsResponse {
    pub clips: Vec<Clip>,
    pub streams: HashMap<String, String>,
}

#[axum::debug_handler]
pub async fn fetch_clips(Json(body): Json<CreateClipsBody>) -> Json<ClipsResponse> {
    let clips = clip::get_all_clips(&body);
    let clips = clip::compile_clips(clips, body.clip_order, body.select_mode);
    let streams: HashMap<String, String> = body
        .markers
        .iter()
        .map(|m| (m.scene.id.clone(), find_stream_url(m).to_string()))
        .collect();
    Json(ClipsResponse { clips, streams })
}

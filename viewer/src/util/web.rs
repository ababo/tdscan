use js_sys::{Array, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, HtmlImageElement, Url};

use crate::defs::IntoResult;
use base::defs::Result;
use base::model;

pub async fn decode_image(image: &model::Image) -> Result<HtmlImageElement> {
    let array = Uint8Array::new_with_length(image.data.len() as u32);
    array.copy_from(&image.data);

    let sequence = Array::new();
    sequence.push(&array);

    let blob = Blob::new_with_u8_array_sequence(&sequence).into_result()?;
    let url = Url::create_object_url_with_blob(&blob).into_result()?;

    let img = HtmlImageElement::new().unwrap();
    img.set_src(&url);

    let res = JsFuture::from(img.decode()).await.into_result();
    Url::revoke_object_url(&url).into_result()?;
    res?;

    Ok(img)
}

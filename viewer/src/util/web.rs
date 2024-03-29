use js_sys::{Array, Promise, Uint8Array};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Blob, Event, EventTarget, HtmlImageElement, Url};

use crate::defs::IntoResult;
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;

pub async fn decode_image(image: &fm::Image) -> Result<HtmlImageElement> {
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

pub async fn next_frame() -> f64 {
    let promise = Promise::new(&mut |resolve, _| {
        window().unwrap().request_animation_frame(&resolve).unwrap();
    });
    JsFuture::from(promise).await.unwrap().as_f64().unwrap()
}

#[allow(clippy::type_complexity)]
pub struct Subscription(
    Option<(EventTarget, String, Closure<dyn Fn(Event) + 'static>)>,
);

impl Subscription {
    fn unsubscribe(mut self) -> Result<()> {
        if let Some((target, r#type, closure)) = self.0.take() {
            target
                .remove_event_listener_with_callback(
                    &r#type,
                    closure.as_ref().unchecked_ref(),
                )
                .into_result()?;
            closure.forget();
            Ok(())
        } else {
            Err(Error::new(
                BadOperation,
                "cannot unsubscribe using inactive subscription".to_string(),
            ))
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if self.0.is_some() {
            let sub = Subscription(Some(self.0.take().unwrap()));
            if let Err(err) = sub.unsubscribe() {
                error!("{}", err);
            }
        }
    }
}

pub fn subscribe<F: Fn(Event) + 'static>(
    target: &EventTarget,
    r#type: &str,
    listener: F,
) -> Result<Subscription> {
    let closure = Closure::wrap(Box::new(listener) as Box<dyn Fn(_)>);
    target
        .add_event_listener_with_callback(
            r#type,
            closure.as_ref().unchecked_ref(),
        )
        .into_result()?;
    Ok(Subscription(Some((
        target.clone(),
        r#type.to_string(),
        closure,
    ))))
}

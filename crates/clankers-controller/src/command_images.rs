//! Session-command image payload translation.
//!
//! Keeps protocol image DTOs at the command boundary and hands the agent a
//! provider-native content vector only after explicit projection.

use clanker_message::Content as ProviderContent;
use clankers_protocol::ImageData;

pub(crate) fn prompt_images_to_provider_content(images: Vec<ImageData>) -> Option<Vec<ProviderContent>> {
    if images.is_empty() {
        return None;
    }

    Some(
        images
            .into_iter()
            .map(|image| ProviderContent::Image {
                source: clanker_message::ImageSource::Base64 {
                    media_type: image.media_type,
                    data: image.data,
                },
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use clanker_message::Content;
    use clanker_message::ImageSource;

    use super::*;

    #[test]
    fn empty_images_project_to_no_provider_content() {
        assert!(prompt_images_to_provider_content(Vec::new()).is_none());
    }

    #[test]
    fn protocol_images_project_to_provider_image_content() {
        let projected = prompt_images_to_provider_content(vec![ImageData {
            media_type: "image/png".to_string(),
            data: "ZmFrZQ==".to_string(),
        }])
        .expect("image content");

        assert!(matches!(
            projected.as_slice(),
            [Content::Image {
                source: ImageSource::Base64 { media_type, data },
            }] if media_type == "image/png" && data == "ZmFrZQ=="
        ));
    }
}

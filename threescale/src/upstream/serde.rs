use anyhow::anyhow;
use core::convert::TryFrom;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};

use super::{Builder, Upstream};

impl<'de> Deserialize<'de> for Upstream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["name", "url", "timeout"];

        enum Field {
            Name,
            Url,
            Timeout,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("name, url or timeout")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "name" => Ok(Field::Name),
                            "url" => Ok(Field::Url),
                            "timeout" => Ok(Field::Timeout),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct UpstreamVisitor;

        impl<'de> Visitor<'de> for UpstreamVisitor {
            type Value = Upstream;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map structure describing an Upstream")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                #[derive(Default)]
                struct UpstreamFields {
                    pub name: Option<String>,
                    pub url: Option<url::Url>,
                    pub timeout: Option<u64>,
                }

                let mut fields = UpstreamFields::default();
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if fields.name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            fields.name = Some(map.next_value()?);
                        }
                        Field::Url => {
                            if fields.url.is_some() {
                                return Err(de::Error::duplicate_field("url"));
                            }
                            fields.url = Some(map.next_value()?);
                        }
                        Field::Timeout => {
                            if fields.timeout.is_some() {
                                return Err(de::Error::duplicate_field("url"));
                            }
                            fields.timeout = Some(map.next_value()?);
                        }
                    }
                }

                let name = fields
                    .name
                    .ok_or_else(|| de::Error::missing_field("name"))?;
                let url = fields.url.ok_or_else(|| de::Error::missing_field("url"))?;

                let upstream_builder = Builder::try_from(url).map_err(|e| {
                    de::Error::custom(anyhow!("failed to deserialize url: {:?}", e))
                })?;
                Ok(upstream_builder.build(&name, fields.timeout))
            }
        }

        deserializer.deserialize_struct("Upstream", FIELDS, UpstreamVisitor)
    }
}

impl Serialize for Upstream {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("Upstream", 3)?;

        st.serialize_field("name", self.name())?;

        let url_s = format!(
            "{}://{}{}",
            self.url.scheme(),
            self.authority(),
            self.path() // FIXME query string?
        );
        st.serialize_field("url", url_s.as_str())?;

        let timeout = self.default_timeout();
        st.serialize_field("timeout", &timeout)?;

        st.end()
    }
}

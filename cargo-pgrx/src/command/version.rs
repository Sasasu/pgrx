use pgrx_pg_config::{PgConfig, Pgrx};

pub(crate) fn pgrx_default(supported_major_versions: &[u16]) -> eyre::Result<Pgrx> {
    let mut pgrx = Pgrx::default();
    rss::PostgreSQLVersionRss::new(supported_major_versions)?
        .into_iter()
        .for_each(|version| pgrx.push(PgConfig::from(version)));

    rss::GreenplumVersionRss::new()?
        .into_iter()
        .for_each(|version| pgrx.push(PgConfig::from(version)));

    Ok(pgrx)
}

mod rss {
    use eyre::WrapErr;
    use owo_colors::OwoColorize;
    use pgrx_pg_config::PgVersion;
    use serde_derive::Deserialize;
    use url::Url;

    use crate::command::build_agent_for_url;
    use pgrx_pg_config::PgDistribution;

    pub(super) struct PostgreSQLVersionRss;

    impl PostgreSQLVersionRss {
        pub(super) fn new(supported_major_versions: &[u16]) -> eyre::Result<Vec<PgVersion>> {
            static VERSIONS_RSS_URL: &str = "https://www.postgresql.org/versions.rss";

            #[derive(Deserialize)]
            struct Rss {
                channel: Channel,
            }

            #[derive(Deserialize)]
            struct Channel {
                item: Vec<Item>,
            }

            #[derive(Deserialize)]
            struct Item {
                title: String,
            }

            let http_client = build_agent_for_url(VERSIONS_RSS_URL)?;
            let response = http_client
                .get(VERSIONS_RSS_URL)
                .call()
                .wrap_err_with(|| format!("unable to retrieve {}", VERSIONS_RSS_URL))?;

            let rss: Rss = match serde_xml_rs::from_str(&response.into_string()?) {
                Ok(rss) => rss,
                Err(e) => return Err(e.into()),
            };

            let mut versions = Vec::new();
            for item in rss.channel.item {
                let title = item.title.trim();
                let mut parts = title.split('.');
                let major = parts.next();
                let minor = parts.next();

                // if we don't have major/minor versions or if they don't parse correctly
                // we'll just assume zero for them and eventually skip them
                let major = major.unwrap().parse::<u16>().unwrap_or_default();
                let minor = minor.unwrap().parse::<u16>().unwrap_or_default();

                if supported_major_versions.contains(&major) {
                    versions.push(
                        PgVersion::new(
                            PgDistribution::PostgresSQL,
                            major,
                            minor,
                            Url::parse(
                                &format!("https://ftp.postgresql.org/pub/source/v{major}.{minor}/postgresql-{major}.{minor}.tar.bz2",
                                         major = major, minor = minor)
                            ).expect("invalid url")
                        ),
                    )
                }
            }

            println!(
                "{} Postgres {}",
                "  Discovered".white().bold(),
                versions.iter().map(|ver| format!("{ver}")).collect::<Vec<_>>().join(", ")
            );

            Ok(versions)
        }
    }

    pub(super) struct GreenplumVersionRss;

    impl GreenplumVersionRss {
        pub(super) fn new() -> eyre::Result<Vec<PgVersion>> {
            static VERSIONS_RSS_URL: &str = "https://github.com/greenplum-db/gpdb/releases.atom";
            const SUPPORTED_MAJOR_VERSIONS: &[u16] = &[7];

            #[derive(Deserialize)]
            struct Feed {
                entry: Vec<Entry>,
            }

            #[derive(Deserialize)]
            struct Entry {
                title: String,
                link: Link,
            }

            #[derive(Deserialize)]
            struct Link {
                href: String,
            }

            let http_client = build_agent_for_url(VERSIONS_RSS_URL)?;
            let response = http_client
                .get(VERSIONS_RSS_URL)
                .call()
                .wrap_err_with(|| format!("unable to retrieve {}", VERSIONS_RSS_URL))?;

            let rss: Feed = match serde_xml_rs::from_str(&response.into_string()?) {
                Ok(rss) => rss,
                Err(e) => return Err(e.into()),
            };

            let mut versions = Vec::new();
            for item in rss.entry {
                let title = item.title.trim();
                let mut parts = title.split('.');
                let major = parts.next();
                let minor = parts.next();

                // if we don't have major/minor versions or if they don't parse correctly
                // we'll just assume zero for them and eventually skip them
                let major = major.unwrap().parse::<u16>().unwrap_or_default();
                let minor = minor.unwrap().parse::<u16>().unwrap_or_default();

                // https://github.com/greenplum-db/gpdb/releases/tag/7.0.0-beta.4
                // https://github.com/greenplum-db/gpdb/archive/refs/tags/7.0.0-beta.4.tar.gz
                let link = item.link.href.trim();
                let link = link.to_string().replace("/releases/tag/", "/archive/refs/tags/");

                if SUPPORTED_MAJOR_VERSIONS.contains(&major) {
                    versions.push(PgVersion::new(
                        PgDistribution::Greenplum,
                        major,
                        minor,
                        Url::parse(&format!("{}.tar.gz", link)).expect("invalid url"),
                    ))
                }
            }

            // stable sort
            versions.sort_by_key(|x| x.semantic_version());
            versions.dedup_by_key(|x| x.semantic_version());

            println!(
                "{} Greenplum {}",
                "  Discovered".white().bold(),
                versions.iter().map(|ver| format!("{ver}")).collect::<Vec<_>>().join(", ")
            );

            Ok(versions)
        }
    }
}

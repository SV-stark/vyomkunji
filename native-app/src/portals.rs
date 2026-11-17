// Government portals the browser extension knows how to sign for.

pub struct Portal {
    pub name: &'static str,
    pub short: &'static str,
    pub domains: &'static [&'static str],
    pub what: &'static str,
    /// Free citizen filings vs. procurement / trade portals.
    pub filings: bool,
}

pub const PORTALS: &[Portal] = &[
    Portal {
        name: "Income Tax e-Filing",
        short: "IT",
        domains: &["incometax.gov.in"],
        what: "ITR filing, forms, notice replies, e-verification",
        filings: true,
    },
    Portal {
        name: "GST Portal",
        short: "GST",
        domains: &["gst.gov.in"],
        what: "GSTR-1, 3B, 9 returns and refunds",
        filings: true,
    },
    Portal {
        name: "MCA21 v3",
        short: "MCA",
        domains: &["mca.gov.in"],
        what: "Company incorporation, ROC filings, director KYC",
        filings: true,
    },
    Portal {
        name: "EPFO Unified Portal",
        short: "PF",
        domains: &["epfindia.gov.in"],
        what: "Employer PF returns, DSC registration, claims",
        filings: true,
    },
    Portal {
        name: "TRACES / TDS",
        short: "TDS",
        domains: &["tdscpc.gov.in", "traces.gov.in"],
        what: "24Q, 26Q TDS returns and certificates",
        filings: true,
    },
    Portal {
        name: "ICEGATE",
        short: "ICE",
        domains: &["icegate.gov.in"],
        what: "Customs import and export filings",
        filings: false,
    },
    Portal {
        name: "DGFT",
        short: "DGF",
        domains: &["dgft.gov.in"],
        what: "Foreign trade licences, certificates of origin",
        filings: false,
    },
    Portal {
        name: "CPPP / eTenders",
        short: "CPP",
        domains: &["eprocure.gov.in", "etenders.gov.in"],
        what: "Central government tenders",
        filings: false,
    },
    Portal {
        name: "IREPS",
        short: "IR",
        domains: &["ireps.gov.in"],
        what: "Railway procurement and auctions",
        filings: false,
    },
    Portal {
        name: "GeM",
        short: "GeM",
        domains: &["gem.gov.in"],
        what: "Government e-Marketplace bids",
        filings: false,
    },
    Portal {
        name: "ESIC & eOffice",
        short: "ES",
        domains: &["esic.gov.in", "eoffice.gov.in"],
        what: "Social security and digital office files",
        filings: false,
    },
    Portal {
        name: "State e-Procurement",
        short: "ST",
        domains: &["nic.in", "tenderwizard.com", "nprocure.com"],
        what: "State tenders (MH, KA, WB, TN, UP, GJ and more)",
        filings: false,
    },
];

/// The portal a request came from, matched on its key or the page address.
pub fn find(portal_key: &str, origin: &str) -> Option<&'static Portal> {
    let host = host_of(origin);
    let key = portal_key.to_ascii_lowercase();
    PORTALS.iter().find(|p| {
        p.domains.iter().any(|d| {
            key == *d
                || key.ends_with(&format!(".{d}"))
                || host == *d
                || host.ends_with(&format!(".{d}"))
        })
    })
}

pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    host_port
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(host_port)
        .to_ascii_lowercase()
}

/// Whether the page that asked is an official government address over https.
/// The browser extension enforces the real boundary; this is for the person to
/// confirm at a glance.
pub fn origin_trusted(origin: &str) -> bool {
    if !origin.starts_with("https://") {
        return false;
    }
    let host = host_of(origin);
    let gov = ["gov.in", "nic.in"];
    let extra = ["tenderwizard.com", "tenderwizard.in", "nprocure.com"];
    gov.iter()
        .chain(extra.iter())
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusts_only_https_government_hosts() {
        assert!(origin_trusted(
            "https://www.incometax.gov.in/iec/foservices/"
        ));
        assert!(origin_trusted("https://mahatenders.gov.in:443/x"));
        assert!(!origin_trusted("http://www.incometax.gov.in/"));
        assert!(!origin_trusted("https://incometax.gov.in.evil.com/"));
        assert!(!origin_trusted(""));
    }

    #[test]
    fn finds_portal_by_key_or_origin() {
        assert_eq!(find("gst.gov.in", "").unwrap().short, "GST");
        assert_eq!(
            find("", "https://services.gst.gov.in/x").unwrap().short,
            "GST"
        );
        assert!(find("unknown", "https://example.com").is_none());
    }
}

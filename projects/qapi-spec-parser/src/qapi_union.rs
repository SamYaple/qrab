use crate::helpers::{kv, qtag};
use crate::{QapiBranches, QapiCond, QapiFeatures, QapiMembers, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserKey<'input> {
    Name(QapiString<'input>),
    Discriminator(QapiString<'input>),
    Data(QapiBranches<'input>),
    Base(QapiUnionBase<'input>),
    If(QapiCond<'input>),
    Features(QapiFeatures<'input>),
}

#[derive(Debug, Clone)]
enum QapiUnionBase<'input> {
    Ref(QapiString<'input>),
    Members(QapiMembers<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiUnion<'input> {
    name: QapiString<'input>,
    data: QapiBranches<'input>,
    base: QapiUnionBase<'input>,
    discriminator: QapiString<'input>,
    r#if: Option<QapiCond<'input>>,
    features: Option<QapiFeatures<'input>>,
}

impl<'input> QapiUnion<'input> {
    /// UNION = { 'union': STRING,
    ///           'base': ( MEMBERS | STRING ),
    ///           'discriminator': STRING,
    ///           'data': BRANCHES,
    ///           '*if': COND,
    ///           '*features': FEATURES }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserKey::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserKey::Features(v)
        });
        let name_parser = map(kv(qtag("union"), QapiString::parse), |v| ParserKey::Name(v));
        let data_parser = map(kv(qtag("data"), QapiBranches::parse), |v| {
            ParserKey::Data(v)
        });
        let discriminator_parser = map(kv(qtag("discriminator"), QapiString::parse), |v| {
            ParserKey::Discriminator(v)
        });
        let base_parser = map(
            kv(
                qtag("base"),
                alt((
                    map(QapiString::parse, |v| QapiUnionBase::Ref(v)),
                    map(QapiMembers::parse, |v| QapiUnionBase::Members(v)),
                )),
            ),
            |v| ParserKey::Base(v),
        );

        let parsers = alt((
            data_parser,
            cond_parser,
            features_parser,
            name_parser,
            base_parser,
            discriminator_parser,
        ));
        delimited(
            qtag("{"),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut r#if = None;
                let mut data = None;
                let mut features = None;
                let mut base = None;
                let mut name = None;
                let mut discriminator = None;
                for i in tokens {
                    match i {
                        ParserKey::If(v) => r#if = Some(v),
                        ParserKey::Discriminator(v) => discriminator = Some(v),
                        ParserKey::Base(v) => base = Some(v),
                        ParserKey::Data(v) => data = Some(v),
                        ParserKey::Name(v) => name = Some(v),
                        ParserKey::Features(v) => features = Some(v),
                    }
                }
                let name = name.expect("union is a required key");
                let data = data.expect("data is a required key");
                let discriminator = discriminator.expect("discriminator is a required key");
                let base = base.expect("base is a required key");
                Self {
                    name,
                    r#if,
                    features,
                    data,
                    base,
                    discriminator,
                }
            }),
            qtag("}"),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "{'union':'SOMENAME',  'discriminator': 'testdiscrim', 'base': 'testbase', 'data':{'branchname':'branchtype'}}",
        "{'union': 'SOMENAME', 'discriminator': 'testdiscrim', 'data': {'branchname':'branchtype'}, 'base': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        r#"{ 'union': 'StatsFilter',
             'base': {
                 'target': 'StatsTarget',
                 '*providers': [ 'StatsRequest' ] },
             'discriminator': 'target',
             'data': { 'vcpu': 'StatsVCPUFilter' } }"#,
        r#"{ 'union': 'ImageInfoSpecific',
             'base': { 'type': 'ImageInfoSpecificKind' },
             'discriminator': 'type',
             'data': {
                 'qcow2': 'ImageInfoSpecificQCow2Wrapper',
                 'vmdk': 'ImageInfoSpecificVmdkWrapper',
                 'luks': 'ImageInfoSpecificLUKSWrapper',
                 'rbd': 'ImageInfoSpecificRbdWrapper',
                 'file': 'ImageInfoSpecificFileWrapper'
              } }"#,
        r#"{ 'union': 'BlockdevOptions',
             'base': { 'driver': 'BlockdevDriver',
                       '*node-name': 'str',
                       '*discard': 'BlockdevDiscardOptions',
                       '*cache': 'BlockdevCacheOptions',
                       '*read-only': 'bool',
                       '*auto-read-only': 'bool',
                       '*force-share': 'bool',
                       '*detect-zeroes': 'BlockdevDetectZeroesOptions' },
             'discriminator': 'driver',
             'data': {
                 'blkdebug':   'BlockdevOptionsBlkdebug',
                 'blklogwrites':'BlockdevOptionsBlklogwrites',
                 'blkverify':  'BlockdevOptionsBlkverify',
                 'blkreplay':  'BlockdevOptionsBlkreplay',
                 'bochs':      'BlockdevOptionsGenericFormat',
                 'cloop':      'BlockdevOptionsGenericFormat',
                 'compress':   'BlockdevOptionsGenericFormat',
                 'copy-before-write':'BlockdevOptionsCbw',
                 'copy-on-read':'BlockdevOptionsCor',
                 'dmg':        'BlockdevOptionsGenericFormat',
                 'file':       'BlockdevOptionsFile',
                 'ftp':        'BlockdevOptionsCurlFtp',
                 'ftps':       'BlockdevOptionsCurlFtps',
                 'gluster':    'BlockdevOptionsGluster',
                 'host_cdrom':  { 'type': 'BlockdevOptionsFile',
                                  'if': 'HAVE_HOST_BLOCK_DEVICE' },
                 'host_device': { 'type': 'BlockdevOptionsFile',
                                  'if': 'HAVE_HOST_BLOCK_DEVICE' },
                 'http':       'BlockdevOptionsCurlHttp',
                 'https':      'BlockdevOptionsCurlHttps',
                 'io_uring':   { 'type': 'BlockdevOptionsIoUring',
                                 'if': 'CONFIG_BLKIO' },
                 'iscsi':      'BlockdevOptionsIscsi',
                 'luks':       'BlockdevOptionsLUKS',
                 'nbd':        'BlockdevOptionsNbd',
                 'nfs':        'BlockdevOptionsNfs',
                 'null-aio':   'BlockdevOptionsNull',
                 'null-co':    'BlockdevOptionsNull',
                 'nvme':       'BlockdevOptionsNVMe',
                 'nvme-io_uring': { 'type': 'BlockdevOptionsNvmeIoUring',
                                    'if': 'CONFIG_BLKIO' },
                 'parallels':  'BlockdevOptionsGenericFormat',
                 'preallocate':'BlockdevOptionsPreallocate',
                 'qcow2':      'BlockdevOptionsQcow2',
                 'qcow':       'BlockdevOptionsQcow',
                 'qed':        'BlockdevOptionsGenericCOWFormat',
                 'quorum':     'BlockdevOptionsQuorum',
                 'raw':        'BlockdevOptionsRaw',
                 'rbd':        'BlockdevOptionsRbd',
                 'replication': { 'type': 'BlockdevOptionsReplication',
                                  'if': 'CONFIG_REPLICATION' },
                 'snapshot-access': 'BlockdevOptionsGenericFormat',
                 'ssh':        'BlockdevOptionsSsh',
                 'throttle':   'BlockdevOptionsThrottle',
                 'vdi':        'BlockdevOptionsGenericFormat',
                 'vhdx':       'BlockdevOptionsGenericFormat',
                 'virtio-blk-vfio-pci':
                               { 'type': 'BlockdevOptionsVirtioBlkVfioPci',
                                 'if': 'CONFIG_BLKIO' },
                 'virtio-blk-vhost-user':
                               { 'type': 'BlockdevOptionsVirtioBlkVhostUser',
                                 'if': 'CONFIG_BLKIO' },
                 'virtio-blk-vhost-vdpa':
                               { 'type': 'BlockdevOptionsVirtioBlkVhostVdpa',
                                 'if': 'CONFIG_BLKIO' },
                 'vmdk':       'BlockdevOptionsGenericCOWFormat',
                 'vpc':        'BlockdevOptionsGenericFormat',
                 'vvfat':      'BlockdevOptionsVVFAT'
             } }"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiUnion::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}

use crate::helpers::{qstring, take_dict, take_kv};
use crate::{take_branches, take_cond, take_features, take_members_or_ref};
use crate::{MembersOrRef, QapiBranches, QapiCond, QapiDocumentation, QapiFeatures};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::IResult;

pub fn take_union(input: &str) -> IResult<&str, QapiUnion<'_>> {
    QapiUnion::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiUnion<'i> {
    pub name: Option<&'i str>,
    pub data: Option<QapiBranches<'i>>,
    pub base: Option<MembersOrRef<'i>>,
    pub discriminator: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiUnion<'i> {
    /// UNION = { 'union': STRING,
    ///           'base': ( MEMBERS | STRING ),
    ///           'discriminator': STRING,
    ///           'data': BRANCHES,
    ///           '*if': COND,
    ///           '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse)(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let (input, _) = take_dict(alt((
            map(take_kv("union", qstring), |v| s.name = Some(v)),
            map(take_kv("data", take_branches), |v| s.data = Some(v)),
            map(take_kv("discriminator", qstring), |v| {
                s.discriminator = Some(v)
            }),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(take_kv("base", take_members_or_ref), |v| s.base = Some(v)),
        )))(input)?;
        Ok((input, s))
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

pub use qapi_runtime::{
    PreconfigSafe, QapiCommand, QapiEvent, QapiFlatUnion, qemu_version, scoped_qemu_version,
    unavailable_type_message, with_version,
};
mod generated;
pub use generated::*;
fn main() {
    assert!(!commands::ChangingFlags::ALLOW_OOB);
    assert!(!commands::ChangingFlags::ALLOW_PRECONFIG);
    assert!(commands::ChangingFlags::SUCCESS_RESPONSE);
    assert!(commands::ChangingFlags::allows_oob((6, 1, 0)));
    assert!(commands::ChangingFlags::allows_preconfig((6, 1, 0)));
    assert!(!commands::ChangingFlags::sends_success_response((6, 1, 0)));
    assert!(!commands::LateFlags::allows_oob((6, 1, 0)));
    assert!(!commands::LateFlags::allows_preconfig((6, 1, 0)));
    assert!(commands::LateFlags::sends_success_response((6, 1, 0)));
    assert!(commands::LateFlags::ALLOW_OOB);
    assert!(commands::LateFlags::ALLOW_PRECONFIG);
    assert!(!commands::LateFlags::SUCCESS_RESPONSE);
    for (name, expected) in [("changing-flags", false), ("late-flags", true)] {
        let command = metadata::COMMANDS
            .iter()
            .find(|command| command.wire_name == name)
            .unwrap();
        assert_eq!(command.allow_oob, expected);
        assert_eq!(command.allow_preconfig, expected);
        assert_eq!(command.success_response, !expected);
    }
    with_version((11, 1, 3), || {
        let _: <commands::ReadImplicitInts as QapiCommand>::Returns = vec![1i64, 2];
        let _: <commands::ReadImplicitNamed as QapiCommand>::Returns = vec![QapiStringGenerated {
            value: "named".into(),
        }];
        let wire = serde_json::json!({"ints":[1,2],"strs":["a"],"direct":[{"value":"direct"}],"named":[{"value":"named"}],"type-direct":["qnum"],"types":["qbool"],"nulls":[null]});
        let value: ImplicitLists = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let wire = serde_json::json!([1, 2]);
        let value: ListAliasChoice = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    for (version, tag) in [
        ((2, 9, 0), "qint"),
        ((2, 9, 0), "qfloat"),
        ((2, 10, 0), "qnum"),
    ] {
        with_version(version, || {
            let wire = serde_json::json!({"kind":tag,"empty":{}});
            let value: BuiltinHolder = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    with_version((2, 9, 0), || {
        assert!(serde_json::to_value(QType::Qnum).is_err());
    });
    with_version((2, 10, 0), || {
        assert!(serde_json::to_value(QType::Qint).is_err());
    });
    let qtype = metadata::ENUMS.get("QType").unwrap();
    assert_eq!(
        qtype
            .variants
            .iter()
            .find(|variant| variant.wire_name == "qint")
            .unwrap()
            .until,
        Some((2, 10, 0))
    );
    assert_eq!(
        qtype
            .variants
            .iter()
            .find(|variant| variant.wire_name == "qnum")
            .unwrap()
            .since,
        (2, 10, 0)
    );
    with_version((11, 1, 3), || {
        let _ = QEmpty { marker: true };
        let wire = serde_json::json!({"kind":"qnum","empty":{}});
        let value: BuiltinHolder = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    assert_eq!(
        metadata::ENUMS.get("KindFlagGenerated").unwrap().wire_name,
        "KindFLAG"
    );
    assert_eq!(
        metadata::ENUMS.get("KindFlag").unwrap().wire_name,
        "KindFlag"
    );
    assert_eq!(
        metadata::ENUMS
            .get("ChangingTypeLegacy2")
            .unwrap()
            .wire_name,
        "ChangingType"
    );
    assert_eq!(
        metadata::ENUMS.get("VariantUnionBranch").unwrap().wire_name,
        "VariantUnionBranch"
    );
    with_version((11, 1, 3), || {
        for tag in ["self", "qapi-self", "qapi-self-alt"] {
            let value: KeywordVariants = serde_json::from_value(serde_json::json!(tag)).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), serde_json::json!(tag));
        }
    });
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            for wire in [serde_json::json!("text"), serde_json::json!([1, 2])] {
                let value: VariantAlt = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(value).unwrap(), wire);
            }
        });
    }
    with_version((11, 1, 3), || {
        assert_eq!(
            serde_json::to_value(VariantAlt::A1Alt(())).unwrap(),
            serde_json::Value::Null
        );
    });
    for name in ["CaseKind", "VariantAlt", "VariantUnionBranch"] {
        let info = metadata::ENUMS.get(name).unwrap();
        let names: std::collections::BTreeSet<_> =
            info.variants.iter().map(|variant| variant.name).collect();
        assert_eq!(names.len(), info.variants.len());
    }
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            for tag in ["fooBAR", "foo_bar"] {
                let value: CaseKind = serde_json::from_value(serde_json::json!(tag)).unwrap();
                assert_eq!(serde_json::to_value(value).unwrap(), serde_json::json!(tag));
                let wire = serde_json::json!({"kind":tag,"value":"payload"});
                let value: VariantUnion = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(value).unwrap(), wire);
            }
        });
    }
    with_version((6, 1, 0), || {
        assert_eq!(
            serde_json::to_value(CaseKind::FooBarAlt2).unwrap(),
            serde_json::json!("foo_bar")
        );
        assert!(serde_json::to_value(CaseKind::FooBarAlt).is_err());
    });
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            let command = commands::LateArgs {
                value: "stable".into(),
            };
            assert_eq!(
                serde_json::to_value(command).unwrap(),
                serde_json::json!({"value":"stable"})
            );
        });
    }
    let _ = LaterType { marker: true };
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            let command = commands::LaterTypeCommand {
                value: "request".into(),
            };
            assert_eq!(
                serde_json::to_value(command).unwrap(),
                serde_json::json!({"value":"request"})
            );
            let command = commands::ChangingTypeCommand {
                value: "request".into(),
            };
            assert_eq!(
                serde_json::to_value(command).unwrap(),
                serde_json::json!({"value":"request"})
            );
        });
    }
    let _: <commands::ReadString as QapiCommand>::Returns = String::from("builtin");
    let _: <commands::ReadNamedString as QapiCommand>::Returns = QapiStringGenerated {
        value: "named".into(),
    };
    let _: <commands::ReadPrefixedString as QapiCommand>::Returns = QapiString { marker: true };
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            let wire = serde_json::json!({"item":{"value":"same"}});
            let value: SpellingHolder = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(value.item.value, "same");
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            let child: SpellingChild =
                serde_json::from_value(serde_json::json!({"value":"base","extra":true})).unwrap();
            assert_eq!(child.value, "base");
        });
    }
    for (version, wire) in [
        ((6, 1, 0), serde_json::json!({"item":"old"})),
        ((11, 1, 3), serde_json::json!({"item":{"value":"new"}})),
    ] {
        with_version(version, || {
            let value: CaseChangingHolder = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    let _ = SwitchInfo { marker: true };
    with_version((11, 1, 3), || {
        let _ = QapiOption { marker: true };
        let _: <commands::ReadUppercase as QapiCommand>::Returns = FooBarGenerated2 {
            first: "upper".into(),
        };
        let _: <commands::ReadCamel as QapiCommand>::Returns = FooBar { second: true };
        let upper: ChildUpper =
            serde_json::from_value(serde_json::json!({"first":"upper","third":7})).unwrap();
        assert_eq!(upper.first, "upper");
        let lower: ChildCamel =
            serde_json::from_value(serde_json::json!({"second":true,"fourth":"lower"})).unwrap();
        assert!(lower.second);
        let upper = serde_json::json!({"kind":"upper","value":"payload"});
        let value: AliasCaseGenerated = serde_json::from_value(upper.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), upper);
        let lower = serde_json::json!({"kind":"lower","value":"payload"});
        let value: AliasCase = serde_json::from_value(lower.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), lower);
    });
    with_version((11, 1, 3), || {
        let wire = serde_json::json!({"kind":"none","U":"named","value":"payload"});
        let value: VirtualOwner = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(value.u, "named");
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let info = metadata::STRUCTS.get("VirtualOwner").unwrap();
        assert!(info.is_union);
        assert_eq!(info.fields.len(), 2);
        assert_eq!(
            metadata::ENUMS.get(info.branch_enum.unwrap()).unwrap().kind,
            metadata::EnumKind::FlatUnionBranch
        );
    });
    with_version((6, 1, 0), || {
        let wire = serde_json::json!({"FooBAR":"first","FooBar":"second","FooBAR_alt":"fourth"});
        let value: CollisionFields = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    with_version((11, 1, 3), || {
        let wire = serde_json::json!({"arguments":"named","extra":42});
        let command: commands::ManualArguments = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(command.arguments, "named");
        assert_eq!(serde_json::to_value(command).unwrap(), wire);
        let command = metadata::COMMANDS
            .iter()
            .find(|command| command.wire_name == "manual-arguments")
            .unwrap();
        assert!(command.args.free_form);
        assert_eq!(command.args.fields.len(), 1);
        assert_eq!(command.args.fields[0].wire_name, "arguments");
    });
    with_version((11, 1, 3), || {
        let wire = serde_json::json!({"FooBAR":"first","FooBar":"second","foo_bar_alt":"third","FooBAR_alt":"fourth"});
        let value: CollisionFields = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let fields = metadata::STRUCTS.get("CollisionFields").unwrap().fields;
        let names: std::collections::BTreeSet<_> = fields.iter().map(|field| field.name).collect();
        assert_eq!(names.len(), fields.len());
    });
    for (version, wire) in [
        ((6, 1, 0), serde_json::json!({"item":"old"})),
        ((11, 1, 3), serde_json::json!({"item":{"value":"new"}})),
    ] {
        with_version(version, || {
            let value: AltHolder = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    with_version((6, 1, 0), || {
        assert_eq!(
            serde_json::to_value(ChangingAltLegacy::Old).unwrap(),
            serde_json::json!("old")
        );
        assert!(serde_json::to_value(CycleShapeLegacy2::Old).is_err());
        let wire = serde_json::json!({"type":"item","data":"old"});
        let value: HistoricalChangingPayload = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    assert_eq!(
        metadata::STRUCTS.get("CycleShapeLegacy").unwrap().name,
        "CycleShape"
    );
    for (version, wire) in [
        ((2, 3, 0), serde_json::json!({"item":"old"})),
        ((6, 1, 0), serde_json::json!({"item":{"value":"middle"}})),
        ((11, 1, 3), serde_json::json!({"item":"new"})),
    ] {
        with_version(version, || {
            let value: CycleHolder = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    let _ = ChangingTypeLegacy { marker: true };
    for (version, wire) in [
        ((6, 1, 0), serde_json::json!({"item":"old"})),
        ((11, 1, 3), serde_json::json!({"item":{"value":"new"}})),
    ] {
        with_version(version, || {
            let value: <commands::QueryChanging as QapiCommand>::Returns =
                serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    for version in [(6, 1, 0), (11, 1, 3)] {
        with_version(version, || {
            let wire = serde_json::json!({"kind":"none","value":"historical"});
            let value: VersionedUnion = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    with_version((11, 1, 3), || {
        let _ = VersionedUnionBranch { marker: true };
        let _ = NestedInlineOuterEmpty { marker: true };
        let command = commands::FooCommandGenerated {
            value: "request".into(),
        };
        assert_eq!(<commands::FooCommandGenerated as QapiCommand>::NAME, "foo");
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"value":"request"})
        );
        let _: <commands::FooCommandGenerated as QapiCommand>::Returns = Foo {
            marker: "response".into(),
        };
        let event = events::SignalEventGenerated2 {
            value: "event".into(),
        };
        assert_eq!(<events::SignalEventGenerated2 as QapiEvent>::NAME, "SIGNAL");
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({"value":"event"})
        );
        let collision: Collision =
            serde_json::from_value(serde_json::json!({"kind":"none","value":"payload"})).unwrap();
        assert_eq!(
            serde_json::to_value(collision).unwrap(),
            serde_json::json!({"kind":"none","value":"payload"})
        );
        let _ = CollisionBranch {
            marker: "declared".into(),
        };
        let _ = CollisionBranchGenerated { marker: 42 };
        let _ = SignalEvent {
            marker: "declared".into(),
        };
        let _ = SignalEventGenerated { marker: 42 };
        let _ = FooCommand { marker: true };
        let _ = ArrayPayloadType { marker: true };
        assert!(metadata::ENUMS.contains_key("CollisionBranchGenerated2"));
        let empty: EmptyUnion = serde_json::from_value(serde_json::json!({"kind":"none"})).unwrap();
        assert_eq!(
            serde_json::to_value(empty).unwrap(),
            serde_json::json!({"kind":"none"})
        );
        assert_eq!(
            metadata::ENUMS.get("EmptyUnionBranch").unwrap().kind,
            metadata::EnumKind::FlatUnionBranch
        );
        let _: Option<<commands::PeerWindowsCommand as QapiCommand>::Returns> = Some(PeerWindows {
            value: "remote".into(),
        });
        for wire in [
            serde_json::json!({}),
            serde_json::json!({"windows":"remote"}),
            serde_json::json!({"posix":"remote"}),
        ] {
            let value: PeerFields = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        }
        for wire in ["windows", "posix"] {
            let value: PeerKind = serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                serde_json::json!(wire)
            );
        }
        // Schema optionality still describes the member when its peer condition holds.
        assert!(
            metadata::STRUCTS
                .get("PeerFields")
                .unwrap()
                .fields
                .iter()
                .all(|field| !field.optional)
        );
        with_version((2, 3, 0), || {
            for wire in [
                serde_json::json!({"outer":{"inner":{"value":"nested"}}}),
                serde_json::json!({"outer":{"inner":{"value":"nested","count":3},"empty":{}},"choice":{"flag":true}}),
            ] {
                let value: NestedInline = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(value.outer.inner.value, "nested");
                assert_eq!(serde_json::to_value(value).unwrap(), wire);
            }
            let declared = NestedInlineOuterInner { declared: true };
            assert_eq!(
                serde_json::to_value(declared).unwrap(),
                serde_json::json!({"declared":true})
            );
            assert!(metadata::STRUCTS.contains_key("NestedInlineOuterInner"));
            assert!(metadata::STRUCTS.contains_key("NestedInlineOuterEmpty"));
        });
        with_version((6, 1, 0), || {
            let wire = serde_json::json!({"visitor":[1,2],"dict":false,"wildcard":null});
            let opaque: OpaqueReferences = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(opaque).unwrap(), wire);
            for field in metadata::STRUCTS.get("OpaqueReferences").unwrap().fields {
                let ty = &metadata::TYPE_REFS[field.ty as usize];
                assert_eq!(ty.kind, metadata::TypeKind::Primitive);
                assert_eq!(ty.prim, metadata::Prim::Any);
            }
            for wire in [
                serde_json::json!({"type":"strings","data":["a"]}),
                serde_json::json!({"type":"objects","data":[{"value":"a"}]}),
                serde_json::json!({"type":"kinds","data":["string"]}),
            ] {
                let value: ArrayPayload = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(value).unwrap(), wire);
            }
        });
        for variant in metadata::ENUMS.get("ArrayPayloadBranch").unwrap().variants {
            assert_eq!(variant.payload_struct, None);
            let list = &metadata::TYPE_REFS[variant.payload_type.unwrap() as usize];
            assert_eq!(list.kind, metadata::TypeKind::List);
            let element = &metadata::TYPE_REFS[list.inner as usize];
            match variant.wire_name {
                "strings" => assert_eq!(element.prim, metadata::Prim::Str),
                "objects" => {
                    assert_eq!(element.kind, metadata::TypeKind::Struct);
                    assert_eq!(element.name, "QapiStringGenerated");
                }
                "kinds" => {
                    assert_eq!(element.kind, metadata::TypeKind::Enum);
                    assert_eq!(element.name, "Kind");
                }
                _ => unreachable!(),
            }
        }
        let command = commands::QapiSelfCommand {
            value: "command".into(),
        };
        assert_eq!(<commands::QapiSelfCommand as QapiCommand>::NAME, "self");
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"value":"command"})
        );
        let _: Option<QapiSelf> = None::<<commands::QapiSelfCommand as QapiCommand>::Returns>;
        let mut wire = serde_json::Map::new();
        for name in [
            "self", "string", "option", "vec", "box", "result", "hashmap", "send", "sync", "sized",
            "drop", "fnonce", "some", "none", "ok", "err", "visitor", "dict",
        ] {
            wire.insert(name.into(), serde_json::json!({"value":name}));
        }
        for name in ["crate", "super", "type", "async"] {
            wire.insert(name.into(), serde_json::json!(name));
        }
        wire.insert("kind".into(), serde_json::json!("string"));
        let wire = serde_json::Value::Object(wire);
        let value: Holder = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(value.kind, Kind::String));
        assert_eq!(value.self_.value, "self");
        assert_eq!(value.crate_, "crate");
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let field = metadata::STRUCTS
            .get("Holder")
            .unwrap()
            .fields
            .iter()
            .find(|field| field.name == "self_")
            .unwrap();
        assert_eq!(field.wire_name, "self");
        let field = metadata::STRUCTS
            .get("Holder")
            .unwrap()
            .fields
            .iter()
            .find(|field| field.name == "string")
            .unwrap();
        let ty = &metadata::TYPE_REFS[field.ty as usize];
        assert_eq!(ty.kind, metadata::TypeKind::Struct);
        assert_eq!(ty.name, "QapiStringGenerated");
        let command = metadata::COMMANDS
            .iter()
            .find(|command| command.wire_name == "read-string")
            .unwrap();
        let ty = &metadata::TYPE_REFS[command.returns.unwrap() as usize];
        assert_eq!(ty.kind, metadata::TypeKind::Primitive);
        assert_eq!(ty.prim, metadata::Prim::Str);
        let command = metadata::COMMANDS
            .iter()
            .find(|command| command.wire_name == "read-strings")
            .unwrap();
        let ty = &metadata::TYPE_REFS[command.returns.unwrap() as usize];
        assert_eq!(ty.kind, metadata::TypeKind::List);
        assert_eq!(
            metadata::TYPE_REFS[ty.inner as usize].prim,
            metadata::Prim::Str
        );
        for variant in metadata::ENUMS
            .get("PeerPayload")
            .unwrap()
            .variants
            .iter()
            .chain(metadata::ENUMS.get("PeerNumber").unwrap().variants)
        {
            let ty = &metadata::TYPE_REFS[variant.payload_type.unwrap() as usize];
            match variant.wire_name {
                "object" => {
                    assert_eq!(ty.kind, metadata::TypeKind::Struct);
                    assert_eq!(variant.payload_struct, Some("QapiStringGenerated"));
                }
                "text" => assert_eq!(ty.prim, metadata::Prim::Str),
                "kind" => {
                    assert_eq!(ty.kind, metadata::TypeKind::Enum);
                    assert_eq!(ty.name, "Kind");
                    assert_eq!(variant.payload_struct, None);
                }
                "number" => assert_eq!(ty.prim, metadata::Prim::I64),
                "nothing" => assert_eq!(ty.prim, metadata::Prim::Null),
                _ => unreachable!(),
            }
        }
    });
}

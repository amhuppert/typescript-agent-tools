#[cfg(test)]
#[test]
fn isolated_attachment_measurement() {
    use crate::attachment_measurement::{LIVE,CALLS,TOTAL};
    use std::{sync::atomic::Ordering,mem::size_of,time::Instant};
    let mut observations = Vec::new();
    for (label,branches,inside,assignments) in [
        ("depth-1999",999,true,0),("depth-2000",1000,false,0),
        ("depth-over",1001,false,0),("flat-4000",0,false,4000),
    ] {
        let mut text = String::from("export function flowLimit(): void {\nlet other: any;\nconst value: number = 1;\n");
        text.push_str(&"if (other) {}\n".repeat(branches));
        text.push_str(&"other = 1;\n".repeat(assignments));
        text.push_str(if inside {"if (other) { /*#query*/value; }\n"} else {"/*#query*/value;\n"});
        text.push_str("}\n");
        let ast=crate::parse(&text).expect("same accepted final flow fixture");
        let arena=ast.nodes().len();
        let reachable=ast.walk().count();
        let calls=CALLS.load(Ordering::Relaxed);
        let total=TOTAL.load(Ordering::Relaxed);
        let live=LIVE.load(Ordering::Relaxed);
        let began=Instant::now();
        let captured=CapturedSyntax::new(ast);
        let capture_ns=began.elapsed().as_nanos();
        let retained=LIVE.load(Ordering::Relaxed)-live;
        let allocated=TOTAL.load(Ordering::Relaxed)-total;
        let allocation_calls=CALLS.load(Ordering::Relaxed)-calls;
        let mut roles=[0usize;3];
        let ambiguous=captured.attachments.values().filter(|value|matches!(value,Attachment::Ambiguous)).count();
        for key in captured.attachments.keys() { roles[match key.role {
            AttachmentRole::Node=>0,AttachmentRole::Name=>1,AttachmentRole::FirstToken=>2,
        }]+=1; }
        observations.push(serde_json::json!({
            "label":label,"sourceBytes":text.len(),"arenaNodes":arena,"reachableNodes":reachable,
            "parentSlots":captured.parents.len(),"parentCapacity":captured.parents.capacity(),
            "attachmentRows":captured.attachments.len(),"attachmentCapacity":captured.attachments.capacity(),
            "nodeRows":roles[0],"nameRows":roles[1],"firstTokenRows":roles[2],"ambiguousRows":ambiguous,
            "keyBytes":size_of::<AttachmentKey>(),"valueBytes":size_of::<Attachment>(),
            "parentSlotBytes":size_of::<Option<NodeId>>(),"retainedRequestedHeapBytes":retained,
            "captureRequestedAllocationBytes":allocated,"captureAllocationCalls":allocation_calls,
            "captureNanosecondsInstrumented":capture_ns
        }));
    }
    println!("ATTACHMENT_MEASUREMENT {}",serde_json::to_string(&observations).unwrap());
}

"""Aggregate separate timing and requested-heap measurements; no pooled timings."""
import collections
import json
import pathlib
import statistics

directory = pathlib.Path(__file__).resolve().parent


def read(name):
    metrics = collections.defaultdict(list)
    inventory = {}
    summary = []
    for line in (directory / name).read_text().splitlines():
        columns = line.split('\t')
        if columns[0] == 'inventory':
            inventory[columns[1]] = dict(zip(
                ['sourceBytes', 'finalSites', 'childLinks', 'primitiveAnnotations', 'arenaNodes', 'termBytes'],
                map(int, columns[2:]),
            ))
        elif columns[0] == 'metric':
            _, file, operation, round, *values = columns
            metrics[file, operation].append(dict(round=int(round), **dict(zip(
                ['nanos', 'incrementalPeakBytes', 'retainedDeltaBytes', 'allocationCalls', 'parses', 'inventorySites', 'queries'],
                map(int, values),
            ))))
        elif columns[0] == 'summary':
            summary = list(map(int, columns[1:]))
    return metrics, inventory, summary


times, inventory, time_summary = read('time.tsv')
memory, memory_inventory, memory_summary = read('memory.tsv')
assert inventory == memory_inventory
assert len(inventory) == 18
assert set(times) == set(memory)
entries = []
for (file, operation), samples in times.items():
    accounted = memory[file, operation]
    assert len(samples) == 5 and len(accounted) == 2
    assert [sample['round'] for sample in samples] == list(range(5))
    assert [sample['round'] for sample in accounted] == list(range(2))
    for key in ['parses', 'inventorySites', 'queries']:
        assert len({sample[key] for sample in samples + accounted}) == 1
    entries.append({
        'file': file,
        'operation': operation,
        'timingRounds': len(samples),
        'timingNanos': {
            'median': statistics.median(sample['nanos'] for sample in samples),
            'min': min(sample['nanos'] for sample in samples),
            'max': max(sample['nanos'] for sample in samples),
        },
        'accountedRounds': len(accounted),
        'accounted': {
            key: {
                'min': min(sample[key] for sample in accounted),
                'max': max(sample[key] for sample in accounted),
            }
            for key in ['incrementalPeakBytes', 'retainedDeltaBytes', 'allocationCalls']
        },
        **{key: samples[0][key] for key in ['parses', 'inventorySites', 'queries']},
    })

result = {
    'inventory': inventory,
    'sourceBytes': sum(value['sourceBytes'] for value in inventory.values()),
    'finalSites': sum(value['finalSites'] for value in inventory.values()),
    'primitiveAnnotations': sum(value['primitiveAnnotations'] for value in inventory.values()),
    'demandedSourceCount': sum(value['primitiveAnnotations'] > 0 for value in inventory.values()),
    'descriptionBytes': sum(value['finalSites'] * value['termBytes'] + value['childLinks'] * 4 for value in inventory.values()),
    'timeSummary': time_summary,
    'memorySummary': memory_summary,
    'metrics': entries,
    'interpretation': 'Timing medians use tracking-disabled binary only; allocation ranges use a distinct instrumented binary. inventorySites in ALL rows includes two undemanded files; parses is the actual count. Source strings and reference descriptions predate incremental measurement.',
}
(directory / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({key: result[key] for key in ['sourceBytes', 'finalSites', 'primitiveAnnotations', 'demandedSourceCount', 'descriptionBytes', 'memorySummary']}))

// TSX: components, attribute callbacks, and nested maps.
declare const items: string[];
declare function useState<T>(value: T): [T, (value: T) => void];

export function List({ title }: { title: string }) {
  const [selected, setSelected] = useState<string | null>(null);
  const onSelect = (item: string) => {
    setSelected(item);
  };
  return (
    <section title={title}>
      <button onClick={() => setSelected(null)}>clear</button>
      {items.map((item) => (
        <span key={item} onClick={() => onSelect(item)}>
          {item}
        </span>
      ))}
    </section>
  );
}

export const Inline = () => <div>{`text`}</div>;

export interface CollapsibleTextProps {
  /*#prop*/maxCollapsedHeight?: number;
  text: string;
}
export function CollapsibleText({ maxCollapsedHeight = 3, text }: CollapsibleTextProps): JSX.Element {
  return <div>{text}{maxCollapsedHeight}</div>;
}
export function read(props: CollapsibleTextProps): number | undefined {
  return props./*#access*/maxCollapsedHeight;
}

import { View } from './definitions';

declare function Card(props: { view: View }): JSX.Element;

export const card = <Card view={{ id: 'd', status: 'open' }} />;

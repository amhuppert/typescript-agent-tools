// Component tags and expression containers are lexical uses; intrinsic tag
// names and attribute names are not, even when a variable shares the name.
function /*@Button:d*/Button(/*@props:d*/props: { /*@!*/label: string }) {
  return < /*@!*/div /*@!*/className={/*@props:u*/props./*@!*/label}>{/*@props:u*/props./*@!*/label}</ /*@!*/div>;
}

const /*@className:d*/className = "x";
const /*@div:d*/div = 1;

export const /*@App:de*/App = () => (
  < /*@!*/span>
    < /*@Button:u*/Button /*@!*/label={/*@className:u*/className} />
    < /*@Button:u*/Button /*@!*/label="text">{/*@div:u*/div}</ /*@Button:u*/Button>
  </ /*@!*/span>
);

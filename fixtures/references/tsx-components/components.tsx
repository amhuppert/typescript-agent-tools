// Component tags and expression containers are lexical uses; intrinsic tag
// names and attribute names are not, even when a variable shares the name.
function /*@Button*/Button(/*@props*/props: { /*@!*/label: string }) {
  return < /*@!*/div /*@!*/className={/*@props*/props./*@!*/label}>{/*@props*/props./*@!*/label}</ /*@!*/div>;
}

const /*@className*/className = "x";
const /*@div*/div = 1;

export const /*@App*/App = () => (
  < /*@!*/span>
    < /*@Button*/Button /*@!*/label={/*@className*/className} />
    < /*@Button*/Button /*@!*/label="text">{/*@div*/div}</ /*@Button*/Button>
  </ /*@!*/span>
);

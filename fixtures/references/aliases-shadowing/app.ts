// The top-level `start` aliases `run`; the parameter `start` shadows it.
import { /*@run:i*/execute as /*@run:i*/start } from "./barrel";
/*@run:u*/start();
function /*@local:d*/local(/*@param:d*/start: () => void) { /*@param:u*/start(); }

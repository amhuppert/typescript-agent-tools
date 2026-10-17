import { Combined } from "./namespace-merged";
type /*#namespaceLocal*/OwnMember = Combined.Local;
type /*#namespaceAliasFallback*/AliasMember = Combined.Remote;
type /*#namespaceMeaningFallback*/AliasByMeaning = Combined.TypeMember;
type /*#namespaceOwnWins*/OwnWins = typeof Combined.shared;
type /*#namespaceMissing*/Missing = Combined.Absent;

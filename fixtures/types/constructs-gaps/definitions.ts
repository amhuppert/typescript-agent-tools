export interface View { id: string; status: 'open' | 'closed'; lane?: string }
import data from './data.json';
export const imported = data;

// The built-in plugins. Their code ships with the app and their data is in the event file.
import type { Plugin } from './api';
import { cashCount } from './cash-count/index';
import { drinks } from './drinks/index';
import { ticketApi } from './ticket-api/index';
import { ticketSheet } from './ticket-sheet/index';
import { whatIf } from './what-if/index';

export const plugins: Plugin[] = [drinks, ticketSheet, ticketApi, whatIf, cashCount];

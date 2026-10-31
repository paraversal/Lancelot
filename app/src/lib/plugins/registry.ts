// The built-in plugins. Their code ships with the app and their data is in the event file.
import type { Plugin } from './api';
import { drinks } from './drinks/index';
import { ticketApi } from './ticket-api/index';
import { ticketSheet } from './ticket-sheet/index';

export const plugins: Plugin[] = [drinks, ticketSheet, ticketApi];

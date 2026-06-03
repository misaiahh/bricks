import { describe, expect, it } from 'vitest';
import { App } from './App';

describe('App', () => {
  it('should create an App instance', () => {
    const app = new App();
    expect(app).toBeDefined();
  });
});

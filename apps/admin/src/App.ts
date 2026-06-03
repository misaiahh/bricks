export class App {
  private container: HTMLElement;

  constructor() {
    this.container = document.createElement('div');
    this.container.className = 'app';
    this.container.innerHTML = `
      <header>
        <h1>Bricks — Admin</h1>
      </header>
      <main>
        <p>Welcome to the Bricks brick management dashboard.</p>
      </main>
    `; // Static content — no user data interpolation
  }

  mount(container: HTMLElement): void {
    container.innerHTML = '';
    container.appendChild(this.container);
  }
}

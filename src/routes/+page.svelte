<main>
  <img src="/logo.png" width="120" height="120" alt="" />
  <h1>DClutter</h1>
  <p>Swipe to clean your files.</p>
  <p class="status">The Windows build is in progress.</p>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-small);
    height: 100%;
    text-align: center;
  }

  h1 {
    margin: var(--space-large) 0 0;
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 600;
  }

  p {
    margin: 0;
    color: var(--text-secondary);
  }

  .status {
    color: var(--text-tertiary);
  }
</style>

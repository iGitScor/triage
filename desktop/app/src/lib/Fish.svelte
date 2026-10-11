<script lang="ts">
let { size = 32 }: { size?: number } = $props()
</script>

<span class="disc" style:width="{size}px" style:height="{size}px">
  <svg viewBox="5 35 88 32" width={size * 0.72} fill="currentColor" aria-hidden="true">
    <path d="M27 51C18 44 10 36 6 36C10 45 10 57 6 66C10 66 18 58 27 51Z" />
    <path fill-rule="evenodd" d="M35 38H79A13 13 0 0 1 79 64H35A13 13 0 0 1 35 38ZM77 51a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z" />
  </svg>
</span>

<style>
  .disc {
    display: inline-grid;
    flex: none;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
  }
</style>

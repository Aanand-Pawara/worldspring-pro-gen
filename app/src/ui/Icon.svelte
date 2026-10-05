<script lang="ts">
  // An icon from icons.ts, in the text colour. With `label` it is announced; otherwise the
  // button or text beside it says what it is.
  import { ICONS, type IconName } from './icons';

  interface Props {
    name: IconName;
    size?: number;
    label?: string;
  }
  let { name, size = 18, label }: Props = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}>{@html ICONS[name]}</svg
>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>

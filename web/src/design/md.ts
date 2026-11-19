/**
 * @material/web element registration + JSX typings.
 *
 * Importing this module registers every Material 3 web
 * component ForgeHub uses. The JSX declarations map the
 * elements' own typed properties onto React props so wrappers
 * stay type-safe. @material/web is the official Material 3
 * component library; its components read the --md-sys-color-*
 * tokens defined in tokens.css.
 */
import '@material/web/button/filled-button.js';
import '@material/web/button/filled-tonal-button.js';
import '@material/web/button/outlined-button.js';
import '@material/web/button/text-button.js';
import '@material/web/iconbutton/icon-button.js';
import '@material/web/textfield/outlined-text-field.js';
import '@material/web/checkbox/checkbox.js';
import '@material/web/switch/switch.js';
import '@material/web/chips/chip-set.js';
import '@material/web/chips/assist-chip.js';
import '@material/web/chips/filter-chip.js';
import '@material/web/dialog/dialog.js';
import '@material/web/menu/menu.js';
import '@material/web/menu/menu-item.js';
import '@material/web/tabs/tabs.js';
import '@material/web/tabs/primary-tab.js';
import '@material/web/fab/fab.js';
import '@material/web/icon/icon.js';
import '@material/web/list/list.js';
import '@material/web/list/list-item.js';
import '@material/web/radio/radio.js';

import type { MdFilledButton } from '@material/web/button/filled-button';
import type { MdFilledTonalButton } from '@material/web/button/filled-tonal-button';
import type { MdOutlinedButton } from '@material/web/button/outlined-button';
import type { MdTextButton } from '@material/web/button/text-button';
import type { MdIconButton } from '@material/web/iconbutton/icon-button';
import type { MdOutlinedTextField } from '@material/web/textfield/outlined-text-field';
import type { MdCheckbox } from '@material/web/checkbox/checkbox';
import type { MdSwitch } from '@material/web/switch/switch';
import type { MdRadio } from '@material/web/radio/radio';
import type { MdChipSet } from '@material/web/chips/chip-set';
import type { MdAssistChip } from '@material/web/chips/assist-chip';
import type { MdFilterChip } from '@material/web/chips/filter-chip';
import type { MdDialog } from '@material/web/dialog/dialog';
import type { MdMenu } from '@material/web/menu/menu';
import type { MdMenuItem } from '@material/web/menu/menu-item';
import type { MdTabs } from '@material/web/tabs/tabs';
import type { MdPrimaryTab } from '@material/web/tabs/primary-tab';
import type { MdSecondaryTab } from '@material/web/tabs/secondary-tab';
import type { MdFab } from '@material/web/fab/fab';
import type { MdLinearProgress } from '@material/web/progress/linear-progress';
import type { MdCircularProgress } from '@material/web/progress/circular-progress';
import type { MdDivider } from '@material/web/divider/divider';
import type { MdIcon } from '@material/web/icon/icon';
import type { MdList } from '@material/web/list/list';
import type { MdListItem } from '@material/web/list/list-item';
import type * as React from 'react';

/**
 * Props for the md-* elements. The elements' full property
 * surface (methods, accessors) doesn't map cleanly onto React
 * props, so this is deliberately permissive: type safety lives
 * in the ForgeHub wrapper components' own APIs, and the md
 * elements are an implementation detail behind them.
 */
type MdProps<T> = React.HTMLAttributes<T> &
  Record<string, unknown> & {
    children?: React.ReactNode;
    ref?: React.Ref<T>;
  };

declare module 'react' {
  // Declaration merging for JSX intrinsic elements requires
  // the namespace form; there is no ES-module equivalent.
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace JSX {
    interface IntrinsicElements {
      'md-filled-button': MdProps<MdFilledButton>;
      'md-filled-tonal-button': MdProps<MdFilledTonalButton>;
      'md-outlined-button': MdProps<MdOutlinedButton>;
      'md-text-button': MdProps<MdTextButton>;
      'md-icon-button': MdProps<MdIconButton>;
      'md-outlined-text-field': MdProps<MdOutlinedTextField>;
      'md-checkbox': MdProps<MdCheckbox>;
      'md-switch': MdProps<MdSwitch>;
      'md-radio': MdProps<MdRadio>;
      'md-chip-set': MdProps<MdChipSet>;
      'md-assist-chip': MdProps<MdAssistChip>;
      'md-filter-chip': MdProps<MdFilterChip>;
      'md-dialog': MdProps<MdDialog>;
      'md-menu': MdProps<MdMenu>;
      'md-menu-item': MdProps<MdMenuItem>;
      'md-tabs': MdProps<MdTabs>;
      'md-primary-tab': MdProps<MdPrimaryTab>;
      'md-secondary-tab': MdProps<MdSecondaryTab>;
      'md-fab': MdProps<MdFab>;
      'md-linear-progress': MdProps<MdLinearProgress>;
      'md-circular-progress': MdProps<MdCircularProgress>;
      'md-divider': MdProps<MdDivider>;
      'md-icon': MdProps<MdIcon>;
      'md-list': MdProps<MdList>;
      'md-list-item': MdProps<MdListItem>;
    }
  }
}

export {};

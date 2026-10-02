import type { GlobalTheme, GlobalThemeOverrides } from 'naive-ui'

/**
 * Naive UI 主题桥接层。
 *
 * 颜色全部取自 `src/style.css` 中的 M3 语义令牌（同一套 hex 值），
 * 这样 Naive 组件与页面 CSS 不会出现两套配色。
 * Naive 只暴露有限的键，缺圆角/高度的组件统一由 `common` 提供。
 */

/** 与 style.css 的 :root 令牌一一对应。 */
interface MdPalette {
    primary: string
    onPrimary: string
    primaryContainer: string
    onPrimaryContainer: string
    primaryHover: string
    primaryPressed: string
    secondary: string
    onSecondary: string
    secondaryContainer: string
    onSecondaryContainer: string
    tertiary: string
    onTertiary: string
    tertiaryContainer: string
    onTertiaryContainer: string
    error: string
    onError: string
    errorContainer: string
    onErrorContainer: string
    errorHover: string
    errorPressed: string
    success: string
    successContainer: string
    warning: string
    warningContainer: string
    surface: string
    onSurface: string
    surfaceVariant: string
    onSurfaceVariant: string
    surfaceContainerLowest: string
    surfaceContainerLow: string
    surfaceContainer: string
    surfaceContainerHigh: string
    surfaceContainerHighest: string
    outline: string
    outlineVariant: string
    inverseSurface: string
    inverseOnSurface: string
    inversePrimary: string
    /** 状态层：hover / pressed 叠加在表面上的中性色 */
    hoverColor: string
    pressedColor: string
    elevation1: string
    elevation2: string
    elevation3: string
}

const light: MdPalette = {
    primary: '#006c4c',
    onPrimary: '#ffffff',
    primaryContainer: '#8cf8c7',
    onPrimaryContainer: '#002114',
    primaryHover: '#0a7a58',
    primaryPressed: '#005139',
    secondary: '#4c6358',
    onSecondary: '#ffffff',
    secondaryContainer: '#cee9da',
    onSecondaryContainer: '#092017',
    tertiary: '#3d6473',
    onTertiary: '#ffffff',
    tertiaryContainer: '#c0e8fb',
    onTertiaryContainer: '#001f29',
    error: '#ba1a1a',
    onError: '#ffffff',
    errorContainer: '#ffdad6',
    onErrorContainer: '#410002',
    errorHover: '#a51616',
    errorPressed: '#8c1010',
    success: '#1e6b45',
    successContainer: '#a5f2c8',
    warning: '#7a5900',
    warningContainer: '#ffdea6',
    surface: '#f5fbf7',
    onSurface: '#171d1a',
    surfaceVariant: '#dbe5dd',
    onSurfaceVariant: '#404943',
    surfaceContainerLowest: '#ffffff',
    surfaceContainerLow: '#eff5f0',
    surfaceContainer: '#e9efea',
    surfaceContainerHigh: '#e3eae4',
    surfaceContainerHighest: '#dee4df',
    outline: '#707972',
    outlineVariant: '#bfc9c1',
    inverseSurface: '#2c322e',
    inverseOnSurface: '#edf2ed',
    inversePrimary: '#70dbab',
    hoverColor: 'rgba(23, 29, 26, 0.06)',
    pressedColor: 'rgba(23, 29, 26, 0.1)',
    elevation1:
        '0 1px 2px rgba(0, 0, 0, 0.3), 0 1px 3px 1px rgba(0, 0, 0, 0.15)',
    elevation2:
        '0 1px 2px rgba(0, 0, 0, 0.3), 0 2px 6px 2px rgba(0, 0, 0, 0.15)',
    elevation3:
        '0 1px 3px rgba(0, 0, 0, 0.3), 0 4px 8px 3px rgba(0, 0, 0, 0.15)',
}

const dark: MdPalette = {
    primary: '#5fdcac',
    onPrimary: '#003825',
    primaryContainer: '#005138',
    onPrimaryContainer: '#7bf8c4',
    primaryHover: '#7fe8bd',
    primaryPressed: '#4cc394',
    secondary: '#b2ccbe',
    onSecondary: '#1e352a',
    secondaryContainer: '#344c40',
    onSecondaryContainer: '#cee9da',
    tertiary: '#a5ccdd',
    onTertiary: '#073543',
    tertiaryContainer: '#24485a',
    onTertiaryContainer: '#c0e8fb',
    error: '#ffb4ab',
    onError: '#690005',
    errorContainer: '#93000a',
    onErrorContainer: '#ffdad6',
    errorHover: '#ffc7c0',
    errorPressed: '#e79a93',
    success: '#8ad6ab',
    successContainer: '#00522f',
    warning: '#f5c06a',
    warningContainer: '#5c4600',
    surface: '#0f1512',
    onSurface: '#dee4df',
    surfaceVariant: '#404943',
    onSurfaceVariant: '#bfc9c1',
    surfaceContainerLowest: '#0a0f0d',
    surfaceContainerLow: '#171d1a',
    surfaceContainer: '#1b211e',
    surfaceContainerHigh: '#262c29',
    surfaceContainerHighest: '#313733',
    outline: '#899389',
    outlineVariant: '#404943',
    inverseSurface: '#dee4df',
    inverseOnSurface: '#2c322e',
    inversePrimary: '#006c4c',
    hoverColor: 'rgba(222, 228, 223, 0.08)',
    pressedColor: 'rgba(222, 228, 223, 0.12)',
    elevation1: '0 1px 3px rgba(0, 0, 0, 0.5), 0 1px 2px rgba(0, 0, 0, 0.4)',
    elevation2:
        '0 1px 3px rgba(0, 0, 0, 0.5), 0 2px 6px 2px rgba(0, 0, 0, 0.4)',
    elevation3:
        '0 1px 3px rgba(0, 0, 0, 0.5), 0 4px 8px 3px rgba(0, 0, 0, 0.45)',
}

const FONT_FAMILY =
    'Roboto, -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif'

/**
 * 依据 M3 角色构建 Naive 主题覆盖。
 *
 * 尺寸策略：默认（medium）控件 48dp 高，满足 Android 触控目标；
 * small/tiny 逐级收敛但仍保持 44dp/32dp，避免密集区域出现小到点不中的控件。
 */
function buildOverrides(p: MdPalette): GlobalThemeOverrides {
    return {
        common: {
            fontFamily: FONT_FAMILY,
            fontSize: '14px',
            fontSizeMini: '11px',
            fontSizeTiny: '12px',
            fontSizeSmall: '14px',
            fontSizeMedium: '14px',
            fontSizeLarge: '16px',
            fontSizeHuge: '18px',
            lineHeight: '1.43',
            fontWeight: '400',
            fontWeightStrong: '500',

            heightTiny: '32px',
            heightSmall: '44px',
            heightMedium: '48px',
            heightLarge: '56px',
            heightHuge: '60px',

            /* 组件圆角只能由 common 提供（Card/Dialog/Menu 没有独立圆角键） */
            borderRadius: '12px',
            borderRadiusSmall: '8px',

            primaryColor: p.primary,
            primaryColorHover: p.primaryHover,
            primaryColorPressed: p.primaryPressed,
            primaryColorSuppl: p.primary,
            infoColor: p.tertiary,
            infoColorHover: p.tertiary,
            infoColorPressed: p.tertiary,
            successColor: p.success,
            successColorHover: p.success,
            successColorPressed: p.success,
            warningColor: p.warning,
            warningColorHover: p.warning,
            warningColorPressed: p.warning,
            errorColor: p.error,
            errorColorHover: p.errorHover,
            errorColorPressed: p.errorPressed,

            textColor1: p.onSurface,
            textColor2: p.onSurface,
            textColor3: p.onSurfaceVariant,
            textColorDisabled: p.outline,
            placeholderColor: p.onSurfaceVariant,
            placeholderColorDisabled: p.outline,

            iconColor: p.onSurfaceVariant,
            iconColorHover: p.onSurface,
            iconColorPressed: p.onSurface,
            iconColorDisabled: p.outline,

            dividerColor: p.outlineVariant,
            borderColor: p.outlineVariant,

            bodyColor: p.surface,
            cardColor: p.surfaceContainerLow,
            modalColor: p.surfaceContainerHigh,
            popoverColor: p.surfaceContainerHigh,
            tableColor: p.surfaceContainerLow,
            tableHeaderColor: p.surfaceContainerHigh,
            inputColor: p.surfaceContainerLowest,
            actionColor: p.surfaceContainer,
            tagColor: p.secondaryContainer,
            avatarColor: p.primaryContainer,
            codeColor: p.surfaceContainerHigh,
            tabColor: p.surfaceContainer,
            invertedColor: p.inverseSurface,
            hoverColor: p.hoverColor,
            pressedColor: p.pressedColor,
            progressRailColor: p.surfaceContainerHighest,
            railColor: p.surfaceContainerHighest,
            scrollbarColor: p.outlineVariant,
            scrollbarColorHover: p.outline,

            boxShadow1: p.elevation1,
            boxShadow2: p.elevation2,
            boxShadow3: p.elevation3,
        },

        Button: {
            /* M3 按钮为全圆角（full shape） */
            borderRadiusTiny: '999px',
            borderRadiusSmall: '999px',
            borderRadiusMedium: '999px',
            borderRadiusLarge: '999px',
            fontWeight: '500',
            fontWeightStrong: '500',
        },

        Input: {
            color: p.surfaceContainerLowest,
            colorFocus: p.surfaceContainerLowest,
            colorDisabled: p.surfaceContainer,
            border: `1px solid ${p.outlineVariant}`,
            borderHover: `1px solid ${p.onSurfaceVariant}`,
            borderFocus: `1px solid ${p.primary}`,
            boxShadowFocus: `inset 0 0 0 1px ${p.primary}`,
            caretColor: p.primary,
            loadingColor: p.primary,
            suffixTextColor: p.onSurfaceVariant,
        },

        Menu: {
            color: 'transparent',
            itemColorHover: p.surfaceContainerHigh,
            itemColorActive: p.secondaryContainer,
            itemColorActiveHover: p.secondaryContainer,
            itemColorActiveCollapsed: p.secondaryContainer,
            itemTextColor: p.onSurfaceVariant,
            itemTextColorHover: p.onSurface,
            itemTextColorActive: p.onSecondaryContainer,
            itemTextColorActiveHover: p.onSecondaryContainer,
            itemIconColor: p.onSurfaceVariant,
            itemIconColorHover: p.onSurface,
            itemIconColorActive: p.onSecondaryContainer,
            itemIconColorActiveHover: p.onSecondaryContainer,
            arrowColor: p.onSurfaceVariant,
            borderColorHorizontal: p.outlineVariant,
            groupTextColor: p.onSurfaceVariant,
        },

        Tabs: {
            barColor: p.primary,
            tabBorderColor: p.outlineVariant,
            tabTextColorLine: p.onSurfaceVariant,
            tabTextColorHoverLine: p.onSurface,
            tabTextColorActiveLine: p.primary,
            tabTextColorBar: p.onSurfaceVariant,
            tabTextColorActiveBar: p.primary,
            tabTextColorHoverBar: p.onSurface,
            tabTextColorSegment: p.onSurfaceVariant,
            tabTextColorActiveSegment: p.onSurface,
            tabTextColorHoverSegment: p.onSurface,
            colorSegment: p.surfaceContainerHigh,
            tabFontWeight: '500',
            tabFontWeightActive: '500',
            paneTextColor: p.onSurface,
        },

        Tag: {
            borderRadiusSmall: '999px',
            color: p.secondaryContainer,
            textColor: p.onSecondaryContainer,
            border: '1px solid transparent',
            colorPrimary: p.primaryContainer,
            textColorPrimary: p.onPrimaryContainer,
            borderPrimary: '1px solid transparent',
            colorSuccess: p.successContainer,
            textColorSuccess: p.onPrimaryContainer,
            colorWarning: p.warningContainer,
            textColorWarning: p.onSurface,
            colorError: p.errorContainer,
            textColorError: p.onErrorContainer,
        },

        Progress: {
            railColor: p.surfaceContainerHighest,
            fillColor: p.primary,
            fillColorSuccess: p.success,
            fillColorWarning: p.warning,
            fillColorError: p.error,
        },

        Empty: {
            textColor: p.onSurfaceVariant,
            extraTextColor: p.onSurfaceVariant,
        },

        Card: {
            color: p.surfaceContainerLow,
            colorModal: p.surfaceContainerLow,
            colorPopover: p.surfaceContainerHigh,
            textColor: p.onSurfaceVariant,
            titleTextColor: p.onSurface,
            borderColor: p.outlineVariant,
            titleFontWeight: '500',
            boxShadow: 'none',
        },

        Dialog: {
            color: p.surfaceContainerHigh,
            titleTextColor: p.onSurface,
            textColor: p.onSurfaceVariant,
            border: `1px solid ${p.outlineVariant}`,
            titleFontWeight: '500',
            iconColor: p.onSurfaceVariant,
        },

        Select: {
            menuBoxShadow: p.elevation2,
        },

        Notification: {
            color: p.inverseSurface,
            textColor: p.inverseOnSurface,
            headerTextColor: p.inverseOnSurface,
            descriptionTextColor: p.inverseOnSurface,
            actionTextColor: p.inversePrimary,
            headerFontWeight: '500',
            boxShadow: p.elevation3,
            iconColor: p.inversePrimary,
        },

        Message: {
            color: p.inverseSurface,
            textColor: p.inverseOnSurface,
            boxShadow: p.elevation3,
            iconColor: p.inversePrimary,
        },

        Table: {
            borderColor: p.outlineVariant,
            thColor: p.surfaceContainerHigh,
            thTextColor: p.onSurfaceVariant,
            tdColor: p.surfaceContainerLow,
            tdTextColor: p.onSurface,
            thFontWeight: '500',
        },
    }
}

// 组件颜色与 style.css 中的浅色页面背景、边框和文字变量对应。
export const lightThemeOverrides: GlobalThemeOverrides = buildOverrides(light)

// 深色模式保持同一套组件尺寸，只调整配色。
export const darkThemeOverrides: GlobalThemeOverrides = buildOverrides(dark)

/** 供按需引用主题对象（当前由 useAppTheme 选择）。 */
export type { GlobalTheme }

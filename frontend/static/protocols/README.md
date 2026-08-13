# Protocol identity assets

These local assets identify third-party protocols in Stargazer's archive-derived UI. They are shown beside the protocol name, never as an endorsement or as Stargazer branding. The marks and trademarks remain the property of their respective owners and are not relicensed under Stargazer's source-code license.

Retrieved on 2026-07-27:

| Local asset | Protocol | Source | Notes |
| --- | --- | --- | --- |
| `aquarius.svg` | Aquarius | [`AquariusDeFi/aqua-network-website`](https://github.com/AquariusDeFi/aqua-network-website/blob/0064472dbbd2cee08f9005cdab12ab6ef1f8796a/src/assets/img/aqua-logo.svg) | Official vector mark; no separate trademark grant found. |
| `blend-v2.svg` | Blend | [`blend-capital/blend-ui`](https://github.com/blend-capital/blend-ui/blob/414c6046f306cda30c634e6f67363ff88e2add28/public/icons/blend_logo.svg) | Unmodified official vector; repository code is MIT, with trademark rights separate. |
| `soroswap.svg` | Soroswap | [`soroswap/frontend`](https://github.com/soroswap/frontend/blob/a80a23a107a44f853112409e8d1af900f31fd81c/src/assets/svg/Logo.svg) | Unmodified official vector; archived repository code is MIT, with trademark rights separate. |
| `phoenix.svg` | Phoenix DeFi Hub | [`Phoenix-Protocol-Group/phoenix-landing`](https://github.com/Phoenix-Protocol-Group/phoenix-landing/blob/7056868b5764c8edd2f4599b88b73d451e8e7b6b/static/images/icon.svg) | Unmodified official app icon; no separate trademark grant found. |
| `defindex.svg` | DeFindex | [`paltalabs/defindex`](https://github.com/paltalabs/defindex/blob/a0fcabfae38f1c7b0c9c2f10ff51d225f8edc2f1/apps/dapp/public/defindex_logo.svg) | Symbol cropped from the official horizontal vector without recoloring. |
| `reflector.svg` | Reflector Network | [`reflector-network/reflector-website`](https://github.com/reflector-network/reflector-website/blob/38e8a77f898468fc4c79faf2f12aa86686a81eaa/static/images/logo.svg) | Unmodified official vector mark. |
| `fxdao.svg` | FxDAO | [`assets.fxdao.io`](https://assets.fxdao.io/brand/FxDAO-logo.svg) | Unmodified asset from FxDAO's official brand CDN. |
| `yieldblox.svg` | YieldBlox | [`script3/yieldblox-docs`](https://github.com/script3/yieldblox-docs/blob/9dccbf28e86f26efd562e23d3ca63b6b9ca52065/.gitbook/assets/YBX%20Logo.svg) | Unmodified icon from the official documentation repository. |

If a source asset changes, update this manifest and review the local SVG before replacing it. Do not silently fetch remote images at runtime: local copies keep the dashboard deterministic and prevent third-party tracking.

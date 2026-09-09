import type { Catalog } from "../domain/catalog";
import { validateCatalog } from "../domain/catalog";

const productionCatalog: Catalog = {
  shoes: [
    {
      id: "puma-deviate-pure-nitro",
      brand: "PUMA",
      model: "Deviate Pure NITRO",
      imageUrl: "/images/shoes/puma-deviate-pure-nitro.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "asics-gel-kayano-33",
      brand: "ASICS",
      model: "GEL-KAYANO 33",
      imageUrl: "/images/shoes/asics-gel-kayano-33.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "hoka-clifton-11",
      brand: "HOKA",
      model: "Clifton 11",
      imageUrl: "/images/shoes/hoka-clifton-11.png",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "hoka-clifton-pro",
      brand: "HOKA",
      model: "Clifton Pro",
      imageUrl: "/images/shoes/hoka-clifton-pro.png",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "hoka-mach-6",
      brand: "HOKA",
      model: "Mach 6",
      imageUrl: "/images/shoes/hoka-mach-6.png",
      releaseDate: {
        month: 3,
        year: 2024,
        sourceUrl: "https://believeintherun.com/shoe-reviews/hoka-mach-6-review/",
      },
    },
    {
      id: "camel-carbon-5k",
      brand: "CAMEL",
      model: "Carbon 5K",
      imageUrl: "/images/shoes/camel-carbon-5k.png",
      releaseDate: { month: 4, year: 2026, sourceUrl: "https://longbridge.com/en/news/281597457" },
    },
    {
      id: "361-popblaze-6",
      brand: "361°",
      model: "POPBLAZE 6.0",
      imageUrl: "/images/shoes/361-popblaze-6.png",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.yorkshire.com/marketplace/d0ff81aa-15b8-4305-9be8-0250e3ed19ca",
      },
    },
    {
      id: "asics-novablast-6",
      brand: "ASICS",
      model: "NOVABLAST 6",
      imageUrl: "/images/shoes/asics-novablast-6.png",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "361-flame-5",
      brand: "361°",
      model: "Flame 5",
      imageUrl: "/images/shoes/361-flame-5.png",
      releaseDate: {
        month: 12,
        year: 2025,
        sourceUrl: "https://vjsneaker.com/collections/361-degrees-flame-5-0",
      },
    },
    {
      id: "361-flame-5-future",
      brand: "361°",
      model: "Flame 5 Future",
      imageUrl: "/images/shoes/361-flame-5-future.png",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl:
          "https://www.shopnings.com/361-flame-5-future-elite-marathon-racing-carbon-plated-shoes",
      },
    },
    {
      id: "adidas-zenboost",
      brand: "adidas",
      model: "ZENBOOST",
      imageUrl: "/images/shoes/adidas-zenboost.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "adidas-adizero-pacer",
      brand: "adidas",
      model: "Adizero Pacer",
      imageUrl: "/images/shoes/adidas-adizero-pacer.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "pan-equiptech-plus",
      brand: "PAN",
      model: "Equiptech+",
      imageUrl: "/images/shoes/pan-equiptech-plus.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl:
          "https://www.facebook.com/panrunningpredator/posts/equiptechfirst-launch-sahagroup-fair-bitec-bangna%EF%B8%8F-27-june-2026200-pairs-only-st/1635277871931672/",
      },
    },
    {
      id: "hoka-skyward-x-2",
      brand: "HOKA",
      model: "Skyward X 2",
      imageUrl: "/images/shoes/hoka-skyward-x-2.png",
      releaseDate: {
        month: 5,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "lining-feidian-6-challenger",
      brand: "Li-Ning",
      model: "Feidian 6 Challenger",
      imageUrl: "/images/shoes/lining-feidian-6-challenger.png",
      releaseDate: { month: 5, year: 2026, sourceUrl: "https://e1981.com/Li-Ning-ARMW003" },
    },
    {
      id: "adidas-supernova-glide",
      brand: "adidas",
      model: "Supernova Glide",
      imageUrl: "/images/shoes/adidas-supernova-glide.png",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl:
          "https://sneakerlegacy.com/shoe/adidas-supernova-glide-cloud-white-lucid-orange/76911",
      },
    },
    {
      id: "asics-gel-cumulus-28",
      brand: "ASICS",
      model: "GEL-CUMULUS 28",
      imageUrl: "/images/shoes/asics-gel-cumulus-28.png",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl:
          "https://www.runnea.co.uk/articles/running-news/asics-cumulus-every-workout-memorable-25811/",
      },
    },
    {
      id: "qiaodan-wind-sl",
      brand: "QIAODAN",
      model: "Wind SL",
      imageUrl: "/images/shoes/qiaodan-wind-sl.png",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl:
          "https://www.facebook.com/yeyelmoto.112/videos/qiaodan-wind-sl-our-breakthrough-ultra-training-shoe-its-built-for-every-single-/2141517159943341/",
      },
    },
    {
      id: "saucony-endorphin-pro-5",
      brand: "Saucony",
      model: "Endorphin Pro 5",
      imageUrl: "/images/shoes/saucony-endorphin-pro-5.png",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "xtep-shyft-pro",
      brand: "XTEP",
      model: "Shyft Pro",
      imageUrl: "/images/shoes/xtep-shyft-pro.png",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.facebook.com/groups/729069763110548/posts/977699358247586",
      },
    },
    {
      id: "hoka-gaviota-6",
      brand: "HOKA",
      model: "Gaviota 6",
      imageUrl: "/images/shoes/hoka-gaviota-6.png",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "kiprun-kipstorm-elite",
      brand: "KIPRUN",
      model: "Kipstorm Elite",
      imageUrl: "/images/shoes/kiprun-kipstorm-elite.png",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://meta-endurance.com/kiprun-kipstorm-elite-preview/",
      },
    },
    {
      id: "qiaodan-wind-4tr",
      brand: "QIAODAN",
      model: "Wind 4TR",
      imageUrl: "/images/shoes/qiaodan-wind-4tr.png",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl:
          "https://www.facebook.com/jayagustin.mendoza/posts/check-it-out-the-new-release-running-shoe-from-qiaodan-this-414-415-mid-month-sa/27240254802266315",
      },
    },
    {
      id: "nike-pegasus-42",
      brand: "Nike",
      model: "Pegasus 42",
      imageUrl: "/images/shoes/nike-pegasus-42.png",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl: "https://nike.com/a/pegasus-42-release-date",
      },
    },
    {
      id: "361-miro-nude-2",
      brand: "361°",
      model: "MIRO NUDE 2",
      imageUrl: "/images/shoes/361-miro-nude-2.png",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl: "https://www.361usa.com/blogs/tips-recipes/miro-nude-2",
      },
    },
    {
      id: "361-miro-nude-sl",
      brand: "361°",
      model: "MIRO NUDE SL",
      imageUrl: "/images/shoes/361-miro-nude-sl.png",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl:
          "https://www.facebook.com/361degreesrun/posts/-mironude-sl-15-is-cominglightweight-responsive-ready-for-every-run-june-15shope/846972018469450/",
      },
    },
    {
      id: "saucony-ride-19",
      brand: "Saucony",
      model: "Ride 19",
      imageUrl: "/images/shoes/saucony-ride-19.png",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "topo-athletic-specter-3",
      brand: "Topo Athletic",
      model: "Spector 3",
      imageUrl: "/images/shoes/topo-athletic-specter-3.jpg",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl: "https://www.roadtrailrun.com/2026/06/topo-specter-3-multi-tester-review-7.html",
      },
    },
    {
      id: "unpause-project-b",
      brand: "Unpause",
      model: "Project B",
      imageUrl: "/images/shoes/unpause-project-b-v3.jpg",
      releaseDate: { month: 5, year: 2026, sourceUrl: "https://www.instagram.com/p/DYI8P91mC2O/" },
    },
    {
      id: "mizuno-hyperwarp-pro",
      brand: "Mizuno",
      model: "Hyperwarp Pro",
      imageUrl: "/images/shoes/mizuno-hyperwarp-pro.jpg",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "anta-c202-g9-2",
      brand: "ANTA",
      model: "C202 G9 2",
      imageUrl: "/images/shoes/anta-c202-g9-2.jpg",
      releaseDate: {
        month: 12,
        year: 2025,
        sourceUrl:
          "https://en.prnasia.com/releases/global/anta-makes-its-debut-at-the-running-event-unveils-new-tech-lineup-with-kenenisa-bekele-514866.shtml",
      },
    },
    {
      id: "skechers-aero-razor",
      brand: "Skechers",
      model: "Aero Razor",
      imageUrl: "/images/shoes/skechers-aero-razor.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl: "https://www.solereview.com/release-date-calendar-for-running-shoes/",
      },
    },
    {
      id: "brooks-ghost-max-3",
      brand: "Brooks",
      model: "Ghost Max 3",
      imageUrl: "/images/shoes/brooks-ghost-max-3.jpg",
      releaseDate: {
        month: 8,
        year: 2025,
        sourceUrl: "https://weartesters.com/brooks-ghost-max-3-performance-review/",
      },
    },
    {
      id: "xtep-one-piece-5",
      brand: "XTEP",
      model: "One Piece 5.0",
      imageUrl: "/images/shoes/xtep-one-piece-5.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl: "https://www.youtube.com/watch?v=Iy3Px1utzt8",
      },
    },
    {
      id: "xtep-260x-3",
      brand: "XTEP",
      model: "260X 3.0",
      imageUrl: "/images/shoes/xtep-260x-3.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl: "https://www.instagram.com/reel/DUze_p6EaG2/",
      },
    },
    {
      id: "brooks-glycerin-max-2",
      brand: "Brooks",
      model: "Glycerin Max 2",
      imageUrl: "/images/shoes/brooks-glycerin-max-2.jpg",
      releaseDate: {
        month: 11,
        year: 2025,
        sourceUrl:
          "https://meta-endurance.com/brooks-glycerin-max-2-review-max-cushion-minimal-changes/",
      },
    },
    {
      id: "qiaodan-feiying-plaid-3-0",
      brand: "QIAODAN",
      model: "Feiying Plaid 3.0",
      imageUrl: "/images/shoes/qiaodan-feiying-plaid-3-0.jpg",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl: "https://qiaodan.asia/products/feiying-plaid3-0",
      },
    },
    {
      id: "saucony-endorphin-elite-3",
      brand: "Saucony",
      model: "Endorphin Elite 3",
      imageUrl: "/images/shoes/saucony-endorphin-elite-3.jpg",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://www.saucony.com/en/endorphin-elite-3/61243U.html",
      },
    },
    {
      id: "saucony-endorphin-azura",
      brand: "Saucony",
      model: "Endorphin Azura",
      imageUrl: "/images/shoes/saucony-endorphin-azura.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl: "https://www.saucony.com/en/endorphin-azura/",
      },
    },
    {
      id: "puma-deviate-nitro-elite-4",
      brand: "PUMA",
      model: "Deviate NITRO Elite 4",
      imageUrl: "/images/shoes/puma-deviate-nitro-elite-4.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl:
          "https://us.puma.com/us/en/pd/deviate-nitro-elite-4-mens-road-running-shoes/312127",
      },
    },
    {
      id: "salomon-s-lab-phantasm-3",
      brand: "Salomon",
      model: "S/LAB Phantasm 3",
      imageUrl: "/images/shoes/salomon-s-lab-phantasm-3.jpg",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://www.salomon.com/en-us/product/s-lab-phantasm-3/li8687.html",
      },
    },
    {
      id: "anta-zone-2-90",
      brand: "ANTA",
      model: "ZONE 2 90",
      imageUrl: "/images/shoes/anta-zone-2-90.jpg",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl: "https://eu.anta.com/products/mens-anta-zone-2-90",
      },
    },
    {
      id: "anta-zone-2-85",
      brand: "ANTA",
      model: "ZONE 2 85",
      imageUrl: "/images/shoes/anta-zone-2-85.jpg",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl: "https://eu.anta.com/products/mens-anta-zone-2-85",
      },
    },
    {
      id: "new-balance-ellipse",
      brand: "New Balance",
      model: "Ellipse",
      imageUrl: "/images/shoes/new-balance-ellipse.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl:
          "https://newbalance.newsmarket.com/latest-news/new-balance-expands-running-portfolio-with-the-launch-of-the-ellipse/s/20f30f29-2585-4372-a509-c391f70dc7e5",
      },
    },
    {
      id: "puma-magmax-nitro-2",
      brand: "PUMA",
      model: "MagMax NITRO 2",
      imageUrl: "/images/shoes/puma-magmax-nitro-2.jpg",
      releaseDate: {
        month: 12,
        year: 2025,
        sourceUrl: "https://us.puma.com/us/en/pd/magmax-nitro-2-mens-running-shoes/312125",
      },
    },
    {
      id: "hoka-mach-7",
      brand: "HOKA",
      model: "Mach 7",
      imageUrl: "/images/shoes/hoka-mach-7.jpg",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl: "https://au.hoka.com/products/u-mach-7-1171904-asrn-asrn",
      },
    },
    {
      id: "asics-superblast-3",
      brand: "ASICS",
      model: "SUPERBLAST 3",
      imageUrl: "/images/shoes/asics-superblast-3.jpg",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl: "https://www.asics.com/us/en-us/superblast-3/p/ANA_1013A177-100.html",
      },
    },
    {
      id: "hoka-cielo-x1-3-0",
      brand: "HOKA",
      model: "Cielo X1 3.0",
      imageUrl: "/images/shoes/hoka-cielo-x1-3-0.jpg",
      releaseDate: {
        month: 1,
        year: 2026,
        sourceUrl: "https://au.hoka.com/products/u-cielo-x1-30-1171927-nyz-nyz",
      },
    },
    {
      id: "hoka-rocket-x-3",
      brand: "HOKA",
      model: "Rocket X 3",
      imageUrl: "/images/shoes/hoka-rocket-x-3.jpg",
      releaseDate: {
        month: 7,
        year: 2025,
        sourceUrl: "https://au.hoka.com/products/u-rocket-x-3-1168724-whi-whi",
      },
    },
    {
      id: "on-cloudmonster-3",
      brand: "On",
      model: "Cloudmonster 3",
      imageUrl: "/images/shoes/on-cloudmonster-3.jpg",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl:
          "https://press.on-running.com/on-unveils-the-next-generation-of-the-cloudmonster-collection-with-the-cloudmonster-3-cloudmonster-3-hyper-and-lightspray-cloudmonster-3-hyper",
      },
    },
    {
      id: "hiracer-hiwings-pro",
      brand: "HIRACER",
      model: "HiWings Pro",
      imageUrl: "/images/shoes/hiracer-hiwings-pro.jpg",
      releaseDate: {
        month: 4,
        year: 2026,
        sourceUrl: "https://hiracers.com/products/hiwings-pro",
      },
    },
    {
      id: "norda-005",
      brand: "norda",
      model: "005",
      imageUrl: "/images/shoes/norda-005.jpg",
      releaseDate: {
        month: 3,
        year: 2025,
        sourceUrl: "https://nordarun.com/products/005-m-neve",
      },
    },
    {
      id: "new-balance-fuelcell-supercomp-elite-v6",
      brand: "New Balance",
      model: "FuelCell SuperComp Elite v6",
      imageUrl: "/images/shoes/nb-sc-elite-v6.jpg",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl:
          "https://www.runningwarehouse.com/New_Balance_SuperComp_Elite_v6/descpage-N6SCEM4.html",
      },
    },
    {
      id: "qiaodan-leili-2-0-gt",
      brand: "QIAODAN",
      model: "Leili 2.0 GT",
      imageUrl: "/images/shoes/qiaodan-leili-2-0-gt.jpg",
      releaseDate: {
        month: 8,
        year: 2026,
        sourceUrl: "https://qiaodan.asia/products/leili-2-0-gt-flame-red",
      },
    },
    {
      id: "mizuno-neo-accera",
      brand: "Mizuno",
      model: "Neo Accera",
      imageUrl: "/images/shoes/mizuno-neo-accera.jpg",
      releaseDate: {
        month: 3,
        year: 2026,
        sourceUrl: "https://usa.mizuno.com/running-mizuno-neo-accera-unisex/",
      },
    },
    {
      id: "puma-deviate-nitro-4",
      brand: "PUMA",
      model: "Deviate NITRO 4",
      imageUrl: "/images/shoes/puma-deviate-nitro-4.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl:
          "https://us.puma.com/us/en/pd/deviate-nitro%E2%84%A2-4-men%27s-running-shoes/312123.html",
      },
    },
    {
      id: "anta-pg7-travel-3",
      brand: "ANTA",
      model: "PG7 Travel 3",
      imageUrl: "/images/shoes/anta-pg7-travel-3.jpg",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://sg.anta.com/products/anta-pg7-travel-3-men",
      },
    },
    {
      id: "mizuno-neo-vista-3",
      brand: "Mizuno",
      model: "Neo Vista 3",
      imageUrl: "/images/shoes/mizuno-neo-vista-3.jpg",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://usa.mizuno.com/running-mizuno-neo-vista-3/",
      },
    },
    {
      id: "mizuno-wave-rider-30",
      brand: "Mizuno",
      model: "Wave Rider 30",
      imageUrl: "/images/shoes/mizuno-wave-rider-30.jpg",
      releaseDate: {
        month: 6,
        year: 2026,
        sourceUrl: "https://usa.mizuno.com/running-wave-rider-30-mens/",
      },
    },
    {
      id: "adidas-adizero-takumi-sen-11",
      brand: "adidas",
      model: "Adizero Takumi Sen 11",
      imageUrl: "/images/shoes/adidas-adizero-takumi-sen-11.jpg",
      releaseDate: {
        month: 7,
        year: 2025,
        sourceUrl: "https://runningshoedb.com/en/shoes/adidas-adizero-takumi-sen-11",
      },
    },
    {
      id: "adidas-hyperboost-edge",
      brand: "adidas",
      model: "HYPERBOOST EDGE",
      imageUrl: "/images/shoes/adidas-hyperboost-edge.jpg",
      releaseDate: {
        month: 8,
        year: 2026,
        sourceUrl: "https://adidas.com/us/release-dates",
      },
    },
    {
      id: "norda-000-x-black-diamond",
      brand: "norda",
      model: "000 x Black Diamond",
      imageUrl: "/images/shoes/norda-000-black-diamond.jpg",
      releaseDate: {
        month: 8,
        year: 2026,
        sourceUrl:
          "https://endurancesportswire.com/norda-and-black-diamond-equipment-launch-long-term-partnership-with-inaugural-alpine-running-collection",
      },
    },
    {
      id: "on-cloudboom-strike-2",
      brand: "On",
      model: "Cloudboom Strike 2",
      imageUrl: "/images/shoes/on-cloudboom-strike-2.jpg",
      releaseDate: {
        month: 7,
        year: 2026,
        sourceUrl:
          "https://press.on-running.com/on-unveils-next-generation-of-cloudboom-marathon-racing-shoes-powered-by-new-cloudtec-sphere-technology",
      },
    },
    {
      id: "new-balance-supercomp-rebel-v1",
      brand: "New Balance",
      model: "SuperComp Rebel v1",
      imageUrl: "/images/shoes/new-balance-supercomp-rebel-v1.jpg",
      releaseDate: {
        month: 8,
        year: 2026,
        sourceUrl:
          "https://houseofheat.co/new-balance/new-balance-sc-elite-v6-sc-rebel-v1-release-date",
      },
    },
    {
      id: "apex-swift-2-0-pro",
      brand: "Apex",
      model: "Swift 2.0 Pro",
      imageUrl: "/images/shoes/apex-swift-2-0-pro.jpg",
      releaseDate: {
        month: 2,
        year: 2026,
        sourceUrl:
          "https://www.facebook.com/Apexswift/videos/apex-swift-20-pro-tuktuk-is-coming-soon23-feb-2026run2paradiseswift20proapexswif/1428583062383909/",
      },
    },
  ],
  reviewers: [
    {
      id: "por-vrr",
      name: "Por VRR",
    },
    {
      id: "jay-runs",
      name: "JAY RUNS",
    },
    {
      id: "papziza",
      name: "กูรูบ้างไม่รู้บ้าง Papziza",
    },
    {
      id: "wing-yang-ngai-hai-uan",
      name: "วิ่งยังไงให้อ้วน",
    },
  ],
  reviews: [
    {
      id: "por-vrr-puma-deviate-pure-nitro",
      reviewerId: "por-vrr",
      shoeId: "puma-deviate-pure-nitro",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "Por VRR เลือก US Men's 10 ซึ่งเป็นไซส์ปกติของเขา หลังจากลอง US Men's 10.5 แล้วพบว่าใส่ได้แต่เหลือพื้นที่มากเกินไป โดย US Men's 10 ให้ความกระชับพอดีกว่าสำหรับวิ่ง",
      sources: ["https://youtu.be/dBHOEIqqijU?si=GIevYLOpPBxukAez"],
    },
    {
      id: "por-vrr-asics-gel-kayano-33",
      reviewerId: "por-vrr",
      shoeId: "asics-gel-kayano-33",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      comparisons: [{ shoeId: "puma-deviate-pure-nitro", stepDelta: 0 }],
      summary:
        "Por VRR สวม US Men's 10 (280 มม.) กับเท้ายาว 265 มม. กว้าง 10 ซม. และระบุว่าเหลือพื้นที่ปลายเท้าประมาณหนึ่งนิ้วโป้ง ขยับนิ้วเท้าได้อิสระและใส่สบายมาก เขาแนะนำว่าเท้าปกติอาจเผื่อความยาว 1 ซม.; หากเท้ากว้างประมาณ 11 ซม. อาจพิจารณาเผื่อ 1.5 ซม. หรือเลือกรุ่นหน้ากว้าง",
      sources: ["https://youtu.be/vUaKke1uVwQ?si=gQlx0eHnzHzztv-1"],
    },
    {
      id: "por-vrr-hoka-clifton-11",
      reviewerId: "por-vrr",
      shoeId: "hoka-clifton-11",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง US Men's 10 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยระบุว่าหน้าเท้าโล่ง ขยับนิ้วได้สบาย และแพลตฟอร์มมั่นคง เขาแนะนำให้เผื่อความยาวประมาณ 1 ซม.; หากเท้ากว้างให้เลือกรุ่น Wide และไม่จำเป็นต้องเผื่อถึง 1.5 ซม. ฟีลนุ่มสบาย เหมาะกับวิ่งชิล วิ่งซ้อมประจำวัน เดินใช้งาน วิ่งในเมือง และวิ่งบนลู่วิ่ง มากกว่าวิ่งเร่งความเร็วจัด ๆ",
      sources: ["https://youtu.be/P7CHOIxCbCk?si=xTkf_gORY6jWw_Cb"],
    },
    {
      id: "por-vrr-hoka-clifton-pro",
      reviewerId: "por-vrr",
      shoeId: "hoka-clifton-pro",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      comparisons: [{ shoeId: "hoka-clifton-11", stepDelta: 0 }],
      summary:
        "Por VRR ทดลอง US Men's 10 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยระบุว่าหน้าเท้าโล่ง ขยับนิ้วได้สบาย และแพลตฟอร์มมั่นคง เช่นเดียวกับ Clifton 11 รุ่นนี้ให้ฟีลแน่นขึ้นเล็กน้อย แต่ตอบสนองกว่าและมี Meta Rocker ชัดกว่า จึงเหมาะกับคนที่อยากได้รองเท้าสบายสำหรับวิ่งชิล และยังหยิบไปซ้อมทำความเร็วได้บ้าง",
      sources: ["https://youtu.be/P7CHOIxCbCk?si=xTkf_gORY6jWw_Cb"],
    },
    {
      id: "por-vrr-camel-carbon-5k",
      reviewerId: "por-vrr",
      shoeId: "camel-carbon-5k",
      triedSize: {
        system: "EU",
        value: "43",
      },
      summary:
        "Por VRR เลือก EU 43 ซึ่งเป็นความยาว 265 มม. สำหรับเท้ายาว 265 มม. กว้างประมาณ 10 ซม. และระบุว่าใส่ลงตัว ขยับนิ้วได้สบาย ไม่รู้สึกแน่น ฟีล C Super Foam ค่อนข้างแน่นแต่มีความนุ่มเล็กน้อยและมั่นคง เหมาะกับรองเท้าซ้อมราคาจับต้องง่ายสำหรับวิ่งชิล วิ่งในเมืองระยะประมาณ 3–10K เดินใช้งาน และออกกำลังกายทั่วไป มากกว่าซ้อมเร่งความเร็ว แม้มี carbon plate โดยน้ำหนักที่พูดถึงหลักอยู่ที่ 278–279 กรัมใน EU 43 แต่มีจุดหนึ่งระบุ 268 กรัม จึงไม่นำไปใช้เป็นสเปกทางการ",
      sources: ["https://youtu.be/rbxEiJ-iGQk?si=_XbPrvACQ1AP1A4b"],
    },
    {
      id: "por-vrr-361-popblaze-6",
      reviewerId: "por-vrr",
      shoeId: "361-popblaze-6",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      summary:
        "Por VRR ระบุว่า 361° POPBLAZE 6.0 ที่เขาทดลองเป็น US Men's 9 / EU 43 สำหรับเท้ายาว 265 มม. และในคลิปแสดงว่ามีพื้นที่ปลายเท้าเหลือประมาณหนึ่งนิ้วโป้ง หลักฐานที่สกัดได้ของคลิปนี้ยืนยันเรื่องไซซ์เป็นหลัก จึงไม่เติมข้อสรุปเรื่องฟีลการวิ่งหรือสเปกทางการที่ transcript ยังยืนยันได้ไม่ครบ",
      sources: ["https://youtu.be/FLVKl8zCPIA?si=dpjfJzIGQ_9ebeAA"],
    },
    {
      id: "por-vrr-asics-novablast-6",
      reviewerId: "por-vrr",
      shoeId: "asics-novablast-6",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง ASICS NOVABLAST 6 ที่ US Men's 10 กับเท้ายาว 265 มม. และระบุว่าบุรองเท้ากระชับติดเท้าโดยไม่มีปัญหาส้นหลุด ลิ้นรองเท้าบางและมีบุ การเผื่อ 1 ซม. หรือ 1.5 ซม. ถูกเก็บเป็นข้อสังเกตใน transcript เท่านั้น จึงยังไม่สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง; คลิปพูดถึงฟีลสนุกและการตอบสนองสำหรับการวิ่งเร็ว แต่ตัวเลข stack, drop และน้ำหนักไม่ถูกนำมาใช้เป็นสเปกทางการ",
      sources: ["https://youtu.be/25qwplWR5nc?si=uAm8ZVSZavUtE618"],
    },
    {
      id: "por-vrr-361-flame-5",
      reviewerId: "por-vrr",
      shoeId: "361-flame-5",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      recommendation: {
        stepDelta: -1,
        basis: "explicit_delta",
      },
      summary:
        "Por VRR ทดลอง 361° Flame 5 ที่ US Men's 9.5 / EU 43 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. มีพื้นที่ปลายเท้าและขยับนิ้วได้สบาย แม้ช่วงกลางเท้าจะแคบนิดหน่อยแต่ไม่รบกวนมาก เขาระบุว่าคนที่ใส่ US 10 ปกติสามารถเลือก US 9.5 แบบเดียวกับเขาได้ และเปรียบเทียบว่า Flame 5 มีแพลตฟอร์มกว้างและใส่สบายกว่า Flame 5 Future เล็กน้อย",
      sources: ["https://youtu.be/2oes5hYVjjw?si=_LLac0kFd02Pl4cy"],
    },
    {
      id: "por-vrr-361-flame-5-future",
      reviewerId: "por-vrr",
      shoeId: "361-flame-5-future",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      recommendation: {
        stepDelta: -1,
        basis: "explicit_delta",
      },
      comparisons: [{ shoeId: "361-flame-5", stepDelta: 0 }],
      summary:
        "Por VRR ทดลอง 361° Flame 5 Future ที่ US Men's 9.5 / EU 43 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. มีพื้นที่ปลายเท้าและขยับนิ้วได้สบาย แต่แพลตฟอร์มแคบกว่า Flame 5 เล็กน้อย จึงรู้สึกว่าช่วงกลางเท้ากระชับกว่าเล็กน้อย หลักฐานในคลิปยืนยันว่าทั้ง Flame 5 และ Flame 5 Future ใช้ขนาด EUR เดิมได้ และผู้ที่ใส่ US 10 ปกติเลือก US 9.5 ได้",
      sources: ["https://youtu.be/2oes5hYVjjw?si=_LLac0kFd02Pl4cy"],
    },
    {
      id: "por-vrr-adidas-zenboost",
      reviewerId: "por-vrr",
      shoeId: "adidas-zenboost",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง adidas ZENBOOST ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และระบุว่าพื้นที่ด้านหน้าสั้นกว่าที่คุ้นเคย แม้เท้ากว้างประมาณ 10 ซม. จะขยับนิ้วได้สบาย เขาแนะนำให้ลองหน้าร้าน โดยเฉพาะการเผื่อ 1.5 ซม. หากต้องการวิ่งระยะยาว; คำแนะนำนี้เป็นเพียงข้อสังเกตเรื่องความยาว ไม่ได้สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง",
      sources: ["https://youtu.be/h-tJJQMnbks?si=7dQIy_ichLZLxnvL"],
    },
    {
      id: "por-vrr-adidas-adizero-pacer",
      reviewerId: "por-vrr",
      shoeId: "adidas-adizero-pacer",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง adidas Adizero Pacer ที่ US Men's 10 กับเท้ายาว 265 มม. โดยระบุว่าขยับนิ้วได้ทั้งสองข้างและทรงกระชับเข้ารูปพอดี ความรู้สึกเป็นโฟมแน่นแต่เด้งและรองเท้ามั่นคง เหมาะกับการขยับความเร็ว เขาพูดถึงการเผื่อ 1 ซม. หรือ 1.5 ซม. ตามระยะวิ่ง เราเลยเก็บไว้เป็นคำอธิบายประกอบ และยังไม่สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง",
      sources: ["https://youtu.be/TCcPiTopqhU?si=4tUOw8p5evIX1xVM"],
    },
    {
      id: "por-vrr-pan-equiptech-plus",
      reviewerId: "por-vrr",
      shoeId: "pan-equiptech-plus",
      summary:
        "Por VRR ทดลอง PAN Equiptech+ ที่ 10 US / 43 EU / 275 cm กับเท้ายาว 265 มม. และบอกว่าปลายรองเท้าสั้นกว่าที่คุ้นเคย ใส่ได้แต่ไม่สบายเท้าเพราะเขาชอบพื้นที่ประมาณ 1.5 ซม.; ความกว้างสำหรับเท้าประมาณ 10 ซม. ขยับนิ้วได้อิสระ เขาชื่นชมฟีลนุ่มเด้งและมองว่าเหมาะกับนักวิ่งเริ่มต้นหรือการซ้อมประจำวัน แต่แนะนำให้ดูความยาวเป็นเซนติเมตรและลองหน้าร้าน จึงยังไม่นับไซซ์นี้เป็นไซซ์อ้างอิง",
      sources: ["https://youtu.be/QWxVo3CJRrU?si=6OSicOtOUHRzNiPq"],
    },
    {
      id: "por-vrr-hoka-skyward-x-2",
      reviewerId: "por-vrr",
      shoeId: "hoka-skyward-x-2",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง HOKA Skyward X 2 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยระบุว่าเป็นไซซ์ที่ใส่สบายที่สุด ขยับนิ้วได้อิสระและบุรอบเท้ากระชับ เขารู้สึกว่าแพลตฟอร์มกว้าง ยืนเต็มเท้าได้มั่นคง และฟีลเหมาะกับการวิ่งยาวแบบนุ่มสบายพร้อมความมั่นคงจากโครงสร้างคาร์บอน; ส่วนคำแนะนำ +1 ซม. หรือ +1.5 ซม. เรายังเก็บไว้เป็นคำอธิบายประกอบ",
      sources: ["https://youtu.be/KIbc-N6wXF0?si=cQMgJLn6Jn49wfdT"],
    },
    {
      id: "por-vrr-lining-feidian-6-challenger",
      reviewerId: "por-vrr",
      shoeId: "lining-feidian-6-challenger",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ระบุว่า Li-Ning Feidian 6 Challenger ที่ US Men's 10 ให้ความสัมพันธ์ใกล้เคียงกับ US 10 ของแบรนด์ญี่ปุ่นหรืออเมริกาที่เขาคุ้นเคย และพูดถึงเท้ากว้างประมาณ 10 ซม. การแนะนำไซซ์ในคลิปผูกกับความยาวเท้าและยกตัวอย่างข้ามแบรนด์ จึงยังไม่สรุปเป็นคำแนะนำเทียบไซซ์ในแคตตาล็อก; ผู้รีวิวยืนยันว่าเป็นรองเท้าคาร์บอนเริ่มต้นที่ใส่สบายและกระชับตามที่เขาเล่าไว้",
      sources: ["https://youtu.be/vmXxBugMlbU?si=DsxLqfN0f0XGVjB7"],
    },
    {
      id: "por-vrr-adidas-supernova-glide",
      reviewerId: "por-vrr",
      shoeId: "adidas-supernova-glide",
      summary:
        "Por VRR มอง adidas Supernova Glide เป็นรองเท้าหนานุ่มที่ให้ซัพพอร์ตและเหมาะกับนักวิ่งมือใหม่ เขาระบุว่าการเผื่อ 1 ซม. น่าจะเพียงพอสำหรับเท้ากว้างประมาณ 10 ซม. แต่ย้ำให้ไปลองที่ adidas Store เพราะยังไม่มีข้อมูลไซซ์ที่ลองจริงครบพอ จึงยังไม่ใส่ไซซ์อ้างอิงหรือคำแนะนำเทียบไซซ์",
      sources: ["https://youtu.be/8E2ve8UMRow?si=8jrfeAyHGqSUy1XK"],
    },
    {
      id: "por-vrr-asics-gel-cumulus-28",
      reviewerId: "por-vrr",
      shoeId: "asics-gel-cumulus-28",
      summary:
        "Por VRR ทดลอง ASICS GEL-CUMULUS 28 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยระบุว่าขยับนิ้วได้ แต่รู้สึกว่าหน้ารองเท้าแอบสั้นแม้เผื่อ 1.5 ซม. จึงแนะนำให้ไปลองที่ร้าน เขาอธิบายฟีลนุ่มและสบายของรุ่นนี้ แต่เพราะยังไม่ได้ยืนยันว่าไซซ์ที่ลองใส่พอดีจริง เราเลยเก็บข้อมูลไซซ์ไว้ในสรุปเท่านั้น",
      sources: ["https://youtu.be/N5lKdPXzY1c?si=UQtr1kOSWa758n9-"],
    },
    {
      id: "por-vrr-qiaodan-wind-sl",
      reviewerId: "por-vrr",
      shoeId: "qiaodan-wind-sl",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง QIAODAN Wind SL ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. พื้นที่ปลายเท้าพอดีและทรง racing flat ค่อนข้างเรียว โดยช่วงอุ้งเท้ากระชับกว่าปกติ เขาบอกว่าสามารถใส่ตรงไซส์ที่คุ้นเคยได้ แต่ก็ระบุว่าขนาดที่เหมาะกับเท้าของเขาอาจเป็น US 9.5 จึงยังไม่สรุปเป็นคำแนะนำเทียบไซซ์; ลักษณะการใช้งานเหมาะกับระยะสั้นมากกว่าระยะไกล และไม่ใช่ตัวเลือกแรกสำหรับมือใหม่",
      sources: ["https://youtu.be/z4kIBotOZDQ?si=weCug41KVMbl-BNI"],
    },
    {
      id: "por-vrr-saucony-endorphin-pro-5",
      reviewerId: "por-vrr",
      shoeId: "saucony-endorphin-pro-5",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 0,
        basis: "explicit_delta",
      },
      summary:
        "Por VRR ทดลอง Saucony Endorphin Pro 5 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และบอกว่าใส่สบาย มีพื้นที่ปลายเท้าและรัดเชือกให้กระชับได้ เขาระบุชัดว่าใครใส่ Pro 4 ที่ไซซ์ใดสามารถต่อ Pro 5 ที่ไซซ์เดิมได้ จึงสรุปได้ว่าใส่ไซซ์เดิมต่อได้; ฟีลที่เล่าคือแน่น มั่นคง และตอบสนองสำหรับการซ้อมความเร็วหรือวันแข่งขัน",
      sources: ["https://youtu.be/TFVCJKp3pDs?si=4H_bMdkXG2_NizxA"],
    },
    {
      id: "por-vrr-xtep-shyft-pro",
      reviewerId: "por-vrr",
      shoeId: "xtep-shyft-pro",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      summary:
        "Por VRR ทดลอง XTEP Shyft Pro ที่ label US Men's 9.5 / EU 43 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยรู้สึกว่าหน้าเท้าแอบสั้นและค่อนข้างแน่น แม้กระชับดี วิ่งเร็วแล้วติดเท้า ความมั่นคงดี และ rocker ช่วยพาไหลไปข้างหน้า การพูดถึงความยาวของไซซ์ XTEP ในคลิปมีหลายแบบ จึงยังไม่สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง",
      sources: ["https://youtu.be/xUvCvhAjShM?si=d7QlWmA2TTvVFZLY"],
    },
    {
      id: "por-vrr-hoka-gaviota-6",
      reviewerId: "por-vrr",
      shoeId: "hoka-gaviota-6",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง HOKA Gaviota 6 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยชื่นชมแพลตฟอร์มที่กว้างและความมั่นคงสูงของรองเท้าสายซัพพอร์ตสำหรับคนเท้าล้ม บุหนานุ่มและยืนหรือเดินนานได้สบาย เขาเสนอว่าเท้าปกติอาจเผื่อประมาณ 1 ซม. ส่วนเท้ากว้างอาจพิจารณา 1.5 ซม.; จึงยังไม่สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง",
      sources: ["https://youtu.be/FfwwRXieH_Q?si=QZ_Hkji5estOBLZi"],
    },
    {
      id: "por-vrr-kiprun-kipstorm-elite",
      reviewerId: "por-vrr",
      shoeId: "kiprun-kipstorm-elite",
      triedSize: {
        system: "EU",
        value: "43",
      },
      summary:
        "Por VRR ทดลอง KIPRUN Kipstorm Elite ที่ EU 43 กับเท้ากว้างประมาณ 10 ซม. เขาระบุว่ารุ่นนี้กว้างขึ้นกว่า Kiprun รุ่นก่อนจนใส่ EU 43 ได้ แต่ด้านหน้ายังเหลือเยอะและไม่มั่นใจว่าการลดเป็น EU 42.5 จะทำให้ด้านข้างแน่นเกินไปหรือไม่ จึงเก็บ EU 43 เป็นไซซ์ที่ลองจริงไว้ก่อนโดยยังไม่สรุปเป็นคำแนะนำ; ตอนวิ่งไม่มีส้นหลุดและโฟมให้ความนุ่มตามที่ผู้รีวิวกล่าวถึง แต่ก็ยังแนะนำให้ลองที่ร้าน",
      sources: ["https://youtu.be/maVj5ELrFCQ?si=Od8BtuoNfsg1JDZx"],
    },
    {
      id: "por-vrr-qiaodan-wind-4tr",
      reviewerId: "por-vrr",
      shoeId: "qiaodan-wind-4tr",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 0,
        basis: "explicit_delta",
      },
      summary:
        "Por VRR ทดลอง QIAODAN Wind 4TR ที่ US Men's 10 / EU 44 / 280 CHN กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. ขยับนิ้วได้สบายและระบุชัดว่าเขาใส่ US 10 มาตลอดในรุ่นของ QIAODAN จึงเลือกตรงไซส์ได้ ฟีลนุ่มสบาย มีความมั่นคงและการตอบสนองพอประมาณ เหมาะกับรองเท้าซ้อมประจำวัน วิ่งชิล วิ่งในเมือง และคนที่เพิ่งเริ่มวิ่ง",
      sources: ["https://youtu.be/WYyLnxRnX7c?si=40QNU-eMreUKz-Rj"],
    },
    {
      id: "por-vrr-nike-pegasus-42",
      reviewerId: "por-vrr",
      shoeId: "nike-pegasus-42",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง Nike Pegasus 42 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยขยับนิ้วได้และรู้สึกว่ารองเท้ากระชับดี แต่ทรงค่อนข้างเรียว เขาแนะนำว่าเท้าปกติอาจเผื่อ 1 ซม. และเท้ากว้างอาจเผื่อ 1.5 ซม.; ส่วนนี้เป็นเพียงข้อสังเกตจากความยาวเท้า ฟีลโดยรวมเป็นรองเท้าซ้อมประจำวันที่มั่นคง ใช้ซ้อมทั่วไปและเดินได้",
      sources: ["https://youtu.be/SBzDSb_n6Ng?si=Sg1lyRvSZxBd5Ua2"],
    },
    {
      id: "por-vrr-361-miro-nude-2",
      reviewerId: "por-vrr",
      shoeId: "361-miro-nude-2",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      recommendation: {
        stepDelta: -1,
        basis: "explicit_delta",
      },
      comparisons: [{ shoeId: "361-miro-nude-sl", stepDelta: 0 }],
      summary:
        "Por VRR ทดลอง 361° MIRO NUDE 2 ที่ US Men's 9.5 / EU 43 กับเท้ายาว 265 มม. และระบุว่าปกติเมื่อใส่ 361° US 10 เขาจะลดมา US 9.5 รุ่นนี้กระชับแต่ยังขยับนิ้วได้สบาย และสบายกว่า MIRO NUDE SL บริเวณหน้าเท้า ฟีลพื้นบาง ตอบสนองไว และสัมผัสพื้นได้ชัด เหมาะกับรองเท้าสาย racing flat, interval, tempo หรือแข่งระยะสั้น มากกว่าวิ่งซ้อมประจำวัน",
      sources: ["https://youtu.be/FRDxpjDfl8o?si=rGVFJ1FMASws1AKk"],
    },
    {
      id: "por-vrr-361-miro-nude-sl",
      reviewerId: "por-vrr",
      shoeId: "361-miro-nude-sl",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      summary:
        "Por VRR ทดลอง 361° MIRO NUDE SL ที่ US Men's 9.5 กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยระบุว่าทรงเรียวและกระชับมาก โดยเฉพาะด้านในของหน้าเท้าจนมีโอกาสอึดอัดหรือเสียดสีเมื่อใส่นาน แม้โฟมแน่นนุ่มเด้งและมั่นคง เหมาะเป็นรองเท้าซ้อมประจำวันที่ใช้วิ่งชิลหรือ tempo ได้ แต่ไม่ใช่ตัวเลือกสำหรับคนที่ต้องการฟีลนุ่มยวบ; การเทียบในคลิปยืนยันว่าใช้ US 9.5 เท่ากับ MIRO NUDE 2",
      sources: ["https://youtu.be/FRDxpjDfl8o?si=rGVFJ1FMASws1AKk"],
    },
    {
      id: "por-vrr-saucony-ride-19",
      reviewerId: "por-vrr",
      shoeId: "saucony-ride-19",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง Saucony Ride 19 ที่ US Men's 10 / 280 มม. กับเท้ายาว 265 มม. โดยรู้สึกว่าหน้าเท้าแอบสั้นแต่ความกว้างยังสบาย และเสนอว่าเผื่อ 1.5 ซม. น่าจะเหมาะกว่า จึงยังไม่สรุปเป็นคำแนะนำว่าต้องขยับไซซ์ขึ้นหรือลง ฟีลเป็นรองเท้าซ้อมประจำวันที่นุ่ม มั่นคง และตอบสนองไม่มาก เหมาะกับวิ่งชิล วิ่งในเมือง นักวิ่งมือใหม่ และการซ้อมระยะยาวตามที่ผู้รีวิวอธิบาย",
      sources: ["https://youtu.be/SonOTcOZ_zo?si=6wFegxJy3GLDfWmS"],
    },
    {
      id: "jay-runs-puma-deviate-pure-nitro",
      reviewerId: "jay-runs",
      shoeId: "puma-deviate-pure-nitro",
      recommendation: {
        stepDelta: 0,
        basis: "explicit_delta",
      },
      summary:
        "JAY RUNS ระบุว่า PUMA Deviate Pure NITRO ใส่ตรงไซส์ได้ ไม่จำเป็นต้องเพิ่มหรือลด ฟิตติ้งกระชับติดเท้าแต่กำลังดี หน้าเท้าไม่บีบและกว้างขึ้นจาก PUMA รุ่นก่อน เหมาะเป็นรองเท้าซ้อมสำหรับวิ่งประจำวันและซ้อมทั่วไป ตั้งแต่วิ่งชิลไปจนถึง tempo พร้อมความนิ่งที่ไม่เร่งผู้วิ่งตลอดเวลา",
      sources: ["https://youtu.be/T3qbLkEpeWI?si=aqrdkmvjunEi4jed"],
    },
    {
      id: "jay-runs-hoka-mach-6",
      reviewerId: "jay-runs",
      shoeId: "hoka-mach-6",
      recommendation: {
        stepDelta: 0,
        basis: "explicit_delta",
      },
      summary:
        "JAY RUNS แนะนำ HOKA Mach 6 ให้ใส่ตรงไซส์ ไม่จำเป็นต้องลดหรือเพิ่ม เป็นรองเท้าซ้อมประจำวันที่ใช้ทำ speedwork ได้ด้วย โฟมมีคาแรคเตอร์นุ่มเฟิร์ม ยุบตัวน้อย เด้งคืนเร็วและตอบสนองไว แม้ไม่มี carbon plate อีกทั้งมีฐานกว้างและให้ความมั่นคงสูง จึงเหมาะทั้งการวิ่งช้า วิ่งประจำวัน และเร่งความเร็ว",
      sources: ["https://youtu.be/yNDDjNEUC_Y?si=Mf8UKZDSWk8S9jJv"],
    },
    {
      id: "por-vrr-topo-athletic-specter-3",
      reviewerId: "por-vrr",
      shoeId: "topo-athletic-specter-3",
      summary:
        "Por VRR ลองสวม Topo Athletic Spector 3 ไซส์ US Men's 10 / EU 44 กับเท้ายาว 265 มม. และระบุว่าหน้าเท้ากว้าง โล่งสบาย ขยับนิ้วได้ดี โดยมองว่าการเผื่อความยาวประมาณ 1 ซม. ก็น่าจะพอ และถ้าเป็นคนที่ใส่รองเท้าหน้าเท้ากว้างระดับ 2E/4E อาจใส่ตรงไซส์ได้ อย่างไรก็ดีคลิปนี้ยังเป็นการลองสวมก่อนรีวิวจริง จึงยังไม่ยืนยันไซซ์อ้างอิงสำหรับการวิ่งแบบจริงจัง",
      sources: ["https://youtu.be/4mNgrBXiHoA?si=A9dOoXXP8Rm6FaJN"],
    },
    {
      id: "por-vrr-unpause-project-b",
      reviewerId: "por-vrr",
      shoeId: "unpause-project-b",
      summary:
        "Por VRR ระบุชัดว่าเป็น Unpause Project B และทดลองใส่ US 10 กับเท้ายาว 265 มม. โดยหน้าเท้าขยับนิ้วได้สบาย พร้อมให้ความเห็นเชิงแนะนำว่าการเผื่อประมาณ 1 ซม. น่าจะกระชับกว่า 1.5 ซม. ซึ่งอาจหลวมไป แต่หลักฐานชุดนี้ยังไม่พอสำหรับการลงเป็นไซซ์อ้างอิงหรือข้อมูลเทียบไซซ์แบบเป็นทางการในแคตตาล็อก",
      sources: ["https://youtu.be/TQkoT5AjuVo?si=J-hjI4pLM218YaNf"],
    },
    {
      id: "por-vrr-mizuno-hyperwarp-pro",
      reviewerId: "por-vrr",
      shoeId: "mizuno-hyperwarp-pro",
      summary:
        "Por VRR ทดลอง Mizuno Hyperwarp Pro ที่ไซซ์ US Men's 10 โดยเล่าว่าทรงหน้าเท้าค่อนข้างสั้นและการเผื่อราว 1.5 ซม.จะใส่สบายกว่า ขณะที่การเผื่อ 1 ซม.จะกระชับกว่า เขายังอธิบายว่าฟีลการสวมใส่ของรุ่นนี้ใกล้กับ Hyperwarp Elite และ Hyperwarp Pure และมองว่ารุ่นนี้เหมาะกับคนที่อยากได้รองเท้าสายซิ่งของ Mizuno ที่ใส่ง่ายกว่าตระกูล Wave Rebellion แต่หลักฐานเรื่องไซซ์ยังเป็นคำอธิบายเชิงเซนติเมตรและความรู้สึกหน้าเท้าสั้น จึงยังไม่สรุปเป็นคำแนะนำขยับไซซ์ในแคตตาล็อก",
      sources: ["https://www.youtube.com/watch?v=o-fMr9xcFiU"],
    },
    {
      id: "por-vrr-anta-c202-g9-2",
      reviewerId: "por-vrr",
      shoeId: "anta-c202-g9-2",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 0,
        basis: "explicit_delta",
      },
      summary:
        "Por VRR รีวิว ANTA C202 G9 2 ในฐานะรองเท้าแข่งคาร์บอนที่เบา ตอบสนองดี และมั่นคงขึ้นกว่ารุ่นก่อน โดยไซซ์ที่เขาลองคือ US Men's 10 และระบุว่าสามารถเลือกไซซ์ US เดิมที่ใส่ประจำได้ ข้อมูลเปรียบเทียบกับรุ่นเดิมและ Zone 2 มีในคลิป แต่ยังไม่ควรลงเป็น comparison ในแคตตาล็อกจนกว่าจะมีรองเท้าอ้างอิงเหล่านั้นใน catalog อย่างเป็นทางการ",
      sources: ["https://www.youtube.com/watch?v=vFBod6ZN6EU"],
    },
    {
      id: "por-vrr-skechers-aero-razor",
      reviewerId: "por-vrr",
      shoeId: "skechers-aero-razor",
      summary:
        "Por VRR รีวิว Skechers Aero Razor โดยทดลองไซซ์ US Men's 10 (280) กับเท้ายาว 265 มม. ทรงค่อนข้างเรียวแต่ยังขยับนิ้วได้ และตัวรองเท้าให้ภาพรวมแนวเบาและซิ่งในตระกูล AERO ข้อมูลเรื่องการเผื่อ 1 ซม. ในคลิปเป็นคำแนะนำตามความยาวเท้า ไม่ใช่ step delta หรือ usual-size statement จึงเก็บไว้ใน summary เท่านั้น",
      sources: ["https://www.youtube.com/watch?v=0WeUGzTbzFE"],
    },
    {
      id: "por-vrr-brooks-ghost-max-3",
      reviewerId: "por-vrr",
      shoeId: "brooks-ghost-max-3",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง Brooks Ghost Max 3 ที่ US Men’s 10 / 280 มม. กับเท้ายาว 265 มม. และเท้ากว้างประมาณ 10 ซม. โดยเหลือพื้นที่ปลายเท้าประมาณหนึ่งนิ้วโป้ง ขยับนิ้วได้สบาย แพลตฟอร์มกว้างและฟีลนุ่มสบายมั่นคง เหมาะกับการเดินเร็ว วิ่งจ๊อก วิ่ง easy และวัน recovery มากกว่างานเร่งความเร็วจัด ๆ",
      sources: ["https://www.youtube.com/watch?v=sliLoLPUmGY"],
    },
    {
      id: "por-vrr-xtep-one-piece-5",
      reviewerId: "por-vrr",
      shoeId: "xtep-one-piece-5",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      summary:
        "Por VRR รีวิว XTEP One Piece 5.0 โดยทดลองไซซ์ US Men’s 9.5 / EU 43 กับเท้ายาวประมาณ 265 มม. และมองว่าฟิตค่อนข้างกระชับแต่ยังใส่วิ่งได้สบาย รุ่นนี้ให้ฟีลลื่นไหล กลิ้งง่าย นุ่มขึ้น และตอบสนองดีขึ้นจากรุ่นก่อน เหมาะเป็นรองเท้าซ้อมราคาย่อมเยาที่ใช้ได้ทั้งวิ่งช้าและเร่งจังหวะบ้าง อย่างไรก็ดีคำแนะนำเรื่องการเผื่อความยาวถูกอธิบายเป็นเซนติเมตร ไม่ใช่ความสัมพันธ์ไซซ์แบบมีลำดับ จึงยังไม่ควรแปลงเป็นคำแนะนำเพิ่มหรือลดไซซ์ในแคตตาล็อก",
      sources: ["https://www.youtube.com/watch?v=ZWalpbw7ZiY"],
    },
    {
      id: "por-vrr-xtep-260x-3",
      reviewerId: "por-vrr",
      shoeId: "xtep-260x-3",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      summary:
        "Por VRR รีวิว XTEP 260X 3.0 โดยลองไซส์ US Men's 9.5 (ระบุคู่กับ EU 43) สำหรับเท้ายาว 265 มม. และบอกว่าหน้าเท้าเหลือประมาณหนึ่งนิ้วโป้ง ขยับนิ้วได้สบายทั้งสองข้าง ลิ้นบาง กระชับ และรอบข้อเท้านุ่มพอดี จึงพอใช้เป็นไซส์ที่เขาใส่วิ่งได้จริงได้ แต่คำอธิบายเรื่องการเลือกไซส์ผูกกับวิธีบวกความยาวและคอนเวนชันไซซ์รองเท้าจีน ไม่ใช่คำแนะนำแบบ ordered size delta หรือ usual-size statement ที่แปลงเป็น stepDelta ได้อย่างปลอดภัย ด้านฟีลการวิ่ง เขามองว่ารุ่นนี้นุ่มและใส่ง่ายขึ้นกว่ารุ่น 2.0 เหมาะกับคนที่อยากเริ่มลองรองเท้าคาร์บอน ใช้ซ้อมทำความเร็วหรือวันแข่งได้",
      sources: ["https://www.youtube.com/watch?v=l6KscmCuts0"],
    },
    {
      id: "por-vrr-brooks-glycerin-max-2",
      reviewerId: "por-vrr",
      shoeId: "brooks-glycerin-max-2",
      summary:
        "Por VRR รีวิว Brooks Glycerin Max 2 ว่าเป็นรองเท้าแนวรองซ้อมและวิ่งยาวที่เน้นความนุ่ม ซัพพอร์ต และความไหลลื่นมากกว่าการทำความเร็ว โดยเขาทดลองคู่ขนาด 280 และบอกว่าหน้ารองเท้าค่อนข้างสั้นแม้เผื่อความยาว 1.5 ซม. แต่ยังขยับนิ้วได้และใช้งานวิ่งยาวได้ดี จึงเก็บข้อสังเกตเรื่องไซซ์ไว้ในสรุปเท่านั้น ยังไม่ยืนยันเป็นไซซ์อ้างอิงหรือคำแนะนำขยับไซซ์ในแคตตาล็อก",
      sources: ["https://www.youtube.com/watch?v=nEVnfN4V-RA"],
    },
    {
      id: "por-vrr-qiaodan-feiying-plaid-3-0",
      reviewerId: "por-vrr",
      shoeId: "qiaodan-feiying-plaid-3-0",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง QIAODAN Feiying Plaid 3.0 ที่ US Men's 10 กับเท้ายาว 265 มม. และมองว่าเหมาะกับงานแข่งหรือ tempo และระยะไม่เกินราว 21 กม. มากกว่าวิ่ง easy หรือ full marathon; คำแนะนำเผื่อความยาว 1–1.5 ซม. จากคลิปเก็บเป็นข้อความโดยไม่แปลงเป็น size step",
      sources: ["https://www.youtube.com/watch?v=hMkEGjdag1I"],
    },
    {
      id: "papziza-saucony-endorphin-elite-3",
      reviewerId: "papziza",
      shoeId: "saucony-endorphin-elite-3",
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "Papziza ทดลอง Endorphin Elite 3 รวมประมาณ 15 กม. หลายความเร็ว และรายงานว่านุ่มเด้ง ผลักส่งดี และมั่นคงขึ้นจาก Elite 2 แต่ยังยุกยิกเมื่อวิ่งช้า; คลิปบอกว่า fit ตรงไซส์แต่ไม่ได้ระบุเลขไซซ์ที่ใส่",
      sources: ["https://www.youtube.com/watch?v=7q0x01bBuRw"],
    },
    {
      id: "jay-runs-saucony-endorphin-elite-3",
      reviewerId: "jay-runs",
      shoeId: "saucony-endorphin-elite-3",
      triedSize: {
        system: "US_M",
        value: "8.5",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "JAY RUNS ทดลอง Saucony Endorphin Elite 3 ที่ US Men's 8.5 และระบุว่า fit ตรงไซส์ นุ่มเด้งและ smooth; ความเห็นนี้เป็นประสบการณ์ของ reviewer ไม่ใช่การรับรองไซซ์สำหรับทุกคน",
      sources: ["https://www.youtube.com/watch?v=zkUvMi1kDts"],
    },
    {
      id: "papziza-saucony-endorphin-azura",
      reviewerId: "papziza",
      shoeId: "saucony-endorphin-azura",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "Papziza ทดลอง Saucony Endorphin Azura ที่ US Men's 9 และมองว่าเป็นรองเท้า smooth-rolling ที่ค่อนข้างแน่นและมั่นคง ใช้ได้กับ easy, tempo และ long run; คำแนะนำความยาวในหน่วย cm ไม่ถูกแปลงเป็น size step",
      sources: ["https://www.youtube.com/watch?v=8uHetfIDCGw"],
    },
    {
      id: "por-vrr-puma-deviate-nitro-elite-4",
      reviewerId: "por-vrr",
      shoeId: "puma-deviate-nitro-elite-4",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง PUMA Deviate NITRO Elite 4 ที่ US Men's 10 และมองว่าเบา ตอบสนองดี มั่นคง และเหมาะกับ speed training หรือวันแข่งมากกว่า daily training; คำแนะนำในคลิปใช้ความยาว cm จึงไม่แปลงเป็น size step",
      sources: ["https://www.youtube.com/watch?v=DI4tDgh5Jes"],
    },
    {
      id: "por-vrr-salomon-s-lab-phantasm-3",
      reviewerId: "por-vrr",
      shoeId: "salomon-s-lab-phantasm-3",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง Salomon S/LAB Phantasm 3 ที่ US Men's 10 และมองว่าเบา นุ่ม มีแรงตอบสนอง เหมาะกับ fast running, tempo และ race มากกว่า easy; เขาตั้งข้อสังเกตเรื่อง heel hold และความมั่นคงเมื่อวิ่งช้า",
      sources: ["https://www.youtube.com/watch?v=IiILY5XaIoU"],
    },
    {
      id: "por-vrr-anta-zone-2-90",
      reviewerId: "por-vrr",
      shoeId: "anta-zone-2-90",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR รีวิว ANTA ZONE 2 90 คู่กับ ZONE 2 85 โดยอธิบายว่า N90 เบา นุ่ม และตอบสนอง เหมาะกับ easy หรือ tempo; ขนาด US Men's 10 เก็บไว้กับรุ่นหลัก N90 ตามบริบทของคลิปเท่านั้น",
      sources: ["https://www.youtube.com/watch?v=EZnesJLsFnU"],
    },
    {
      id: "por-vrr-anta-zone-2-85",
      reviewerId: "por-vrr",
      shoeId: "anta-zone-2-85",
      summary:
        "Por VRR รีวิว ANTA ZONE 2 85 คู่กับ ZONE 2 90 และมองว่า N85 แน่น หนัก และมั่นคงกว่า เหมาะกับ zone-2, easy และ long run; คลิปไม่ได้แยกเลขไซซ์ที่สวมสำหรับ N85 จึงไม่เติม fit size",
      sources: ["https://www.youtube.com/watch?v=EZnesJLsFnU"],
    },
    {
      id: "por-vrr-new-balance-ellipse",
      reviewerId: "por-vrr",
      shoeId: "new-balance-ellipse",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง New Balance Ellipse ที่ US Men's 10 และมองว่านุ่ม มั่นคง เหมาะกับ casual หรือ easy มากกว่างานเร็ว; คลิปเปรียบเทียบกับ 1080 แต่ไม่ได้ใช้ชื่อรุ่นเปรียบเทียบเหล่านั้นเป็น shoe record ใหม่",
      sources: ["https://www.youtube.com/watch?v=4eQ8-FPaVKE"],
    },
    {
      id: "por-vrr-puma-magmax-nitro-2",
      reviewerId: "por-vrr",
      shoeId: "puma-magmax-nitro-2",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง PUMA MagMax NITRO 2 ที่ US Men's 10 และมองว่าเบาลงจากรุ่นก่อน แต่ยังเป็น daily trainer ที่เน้นความมั่นคง เหมาะกับ easy หรือ long run มากกว่าการเร่งความเร็ว",
      sources: ["https://www.youtube.com/watch?v=1dAtPM5LFdo"],
    },
    {
      id: "por-vrr-hoka-mach-7",
      reviewerId: "por-vrr",
      shoeId: "hoka-mach-7",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง HOKA Mach 7 ที่ US Men's 10 และรายงานว่าโฟมนุ่มขึ้นเล็กน้อยแต่ยังแน่นเด้ง ตอบสนองและมั่นคง ใช้ได้ทั้ง daily, fast training และ easy run",
      sources: ["https://www.youtube.com/watch?v=uRJ0pq8f52s"],
    },
    {
      id: "por-vrr-asics-superblast-3",
      reviewerId: "por-vrr",
      shoeId: "asics-superblast-3",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR เปรียบเทียบและรีวิว ASICS SUPERBLAST 3 จากสองคลิป โดยระบุว่าให้ฟีลนุ่มเด้งและยังทำความเร็วได้ แต่เหมาะกับ easy หรือ recovery มากกว่างานแข่ง; คำแนะนำความยาวใน cm ไม่ถูกแปลงเป็น size step",
      sources: [
        "https://www.youtube.com/watch?v=cTw42EPri-g",
        "https://www.youtube.com/watch?v=JJ96Vlb8mKg",
      ],
    },
    {
      id: "por-vrr-hoka-cielo-x1-3-0",
      reviewerId: "por-vrr",
      shoeId: "hoka-cielo-x1-3-0",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง HOKA Cielo X1 3.0 ที่ US Men's 10 และมองว่าเป็นรองเท้า racing ที่นุ่มเด้งและ aggressive แต่มีข้อสังเกตเรื่องความมั่นคงเมื่อเข้าโค้งหรือวิ่งช้า",
      sources: ["https://www.youtube.com/watch?v=9wLfcfaIQIE"],
    },
    {
      id: "por-vrr-hoka-rocket-x-3",
      reviewerId: "por-vrr",
      shoeId: "hoka-rocket-x-3",
      summary:
        "Por VRR พูดถึง HOKA Rocket X 3 ในบริบทเปรียบเทียบกับ Cielo X1 3.0; ข้อมูลจากคลิปนี้ไม่ได้แยกเลขไซซ์ที่สวมของ Rocket X 3 จึงเก็บเฉพาะ observation และไม่ย้าย fit size จากอีกรุ่น",
      sources: ["https://www.youtube.com/watch?v=9wLfcfaIQIE"],
    },
    {
      id: "por-vrr-on-cloudmonster-3",
      reviewerId: "por-vrr",
      shoeId: "on-cloudmonster-3",
      summary:
        "Por VRR รีวิว On Cloudmonster 3 และเปรียบเทียบกับ Monster 2 โดยมองว่าฟีลแน่นขึ้นและเหมาะกับ easy, long หรือ city run มากกว่างานแข่งเร็ว; คลิปไม่มีเลขไซซ์รองเท้าที่สวมจึงไม่เติม tried size",
      sources: ["https://www.youtube.com/watch?v=EJV5n8T42lM"],
    },
    {
      id: "por-vrr-saucony-endorphin-azura",
      reviewerId: "por-vrr",
      shoeId: "saucony-endorphin-azura",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR ทดลอง Saucony Endorphin Azura ที่ US Men's 10 และอธิบายว่าเป็น daily trainer/speed training ที่ใช้โฟมเต็มก้อนโดยไม่มีแผ่นคาร์บอน เหมาะกับ easy, tempo และ long run; คำแนะนำความยาวใน cm ไม่ถูกแปลงเป็น size step",
      sources: ["https://www.youtube.com/watch?v=WTXL2f_gti0"],
    },
    {
      id: "papziza-hiracer-hiwings-pro",
      reviewerId: "papziza",
      shoeId: "hiracer-hiwings-pro",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      summary:
        "Papziza รีวิว HIRACER HiWings Pro หลังใช้งานและมีคลิปแกะกล่องของรุ่นเดียวกันประกอบ โดย fit evidence ที่ยืนยันได้คือ US Men's 9; ไม่มี discrete size-step recommendation เพิ่มเติม",
      sources: [
        "https://www.youtube.com/watch?v=ExgAQuOa3j0",
        "https://www.youtube.com/watch?v=xYaDr4mkm3g",
      ],
    },
    {
      id: "papziza-norda-005",
      reviewerId: "papziza",
      shoeId: "norda-005",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 1,
        basis: "explicit_delta",
      },
      summary:
        "Papziza ทดลอง norda 005 ที่ US Men's 10 และให้คำแนะนำเผื่อขึ้น 1 size ตามที่พูดตรง ๆ ในคลิป; คำแนะนำนี้เป็นหลักฐานเฉพาะของ reviewer ไม่ใช่การแปลงจากความยาวเท้า",
      sources: ["https://www.youtube.com/watch?v=SLEjeaSZ9dQ"],
    },
    {
      id: "papziza-asics-novablast-6",
      reviewerId: "papziza",
      shoeId: "asics-novablast-6",
      triedSize: {
        system: "US_M",
        value: "9.5",
      },
      summary:
        "Papziza ทดลอง ASICS NOVABLAST 6 ที่ US Men's 9.5 กับเท้ายาว 26 ซม. และระบุว่า fit โดยรวมพอดี แต่หน้าเท้าอาจแน่นสำหรับเท้า 2E หรือเท้าที่มีเนื้อเยอะ; เหมาะกับ easy และ long run ตามประสบการณ์ในคลิป",
      sources: ["https://www.youtube.com/watch?v=vPKXjxIUHuk"],
    },
    {
      id: "papziza-hoka-clifton-pro",
      reviewerId: "papziza",
      shoeId: "hoka-clifton-pro",
      summary:
        "Papziza รีวิว HOKA Clifton Pro หลังใช้ประมาณ 80–100 กม. และมองว่าเป็นรองเท้า cushion/support ที่มั่นคง ใส่ easy, recovery และ long run ได้ แต่ไม่เด่นด้าน tempo เพราะน้ำหนัก; คลิปไม่มีเลขไซซ์ที่สวม",
      sources: ["https://www.youtube.com/watch?v=sgbcYUi_IgU"],
    },
    {
      id: "papziza-new-balance-fuelcell-supercomp-elite-v6",
      reviewerId: "papziza",
      shoeId: "new-balance-fuelcell-supercomp-elite-v6",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      summary:
        "Papziza ทดลอง New Balance FuelCell SuperComp Elite v6 ที่ US Men's 9 และพบว่าสั้น โดยเฉพาะปลายเท้า พร้อมแนะนำให้ลอง US Men's 9.5; คลิปยังไม่ได้ทดสอบวิ่งจริง จึงเก็บคำแนะนำ half-size เป็นข้อความเท่านั้น",
      sources: ["https://www.youtube.com/watch?v=0mMyN7PfqWI"],
    },
    {
      id: "papziza-qiaodan-leili-2-0-gt",
      reviewerId: "papziza",
      shoeId: "qiaodan-leili-2-0-gt",
      triedSize: {
        system: "US_M",
        value: "11",
      },
      summary:
        "Papziza ทดลอง QIAODAN Leili 2.0 GT หรือ TG 2.0 สองครั้ง ทั้งวิ่งช้า เดิน และเร่งสั้น ๆ โดยสรุปว่าโฟมนุ่มและใช้งานได้หลากหลาย แต่ช่วงวิ่งช้าอาจมีอาการยุกยิกเล็กน้อย",
      sources: ["https://www.youtube.com/watch?v=J0EAqEvuzsQ"],
    },
    {
      id: "papziza-hoka-clifton-11",
      reviewerId: "papziza",
      shoeId: "hoka-clifton-11",
      summary:
        "Papziza รีวิว HOKA Clifton 11 ว่าเป็นรองเท้า daily ที่สบายและมั่นคง ใช้เดินหรือวิ่งประจำวันได้ และให้ความรู้สึกกลิ้งส่งเท้าได้ดี; คลิปนี้ไม่มีเลขไซซ์ที่สวม",
      sources: ["https://www.youtube.com/watch?v=Wuf5VCz3eEY"],
    },
    {
      id: "papziza-mizuno-neo-accera",
      reviewerId: "papziza",
      shoeId: "mizuno-neo-accera",
      summary:
        "Papziza รีวิว Mizuno Neo Accera ว่าเป็น trail shoe ที่ใช้ได้หลากหลาย แต่โฟมที่นุ่มทำให้ต้องระวังอาการบิดหรือไม่มั่นคงบนทางหินเทคนิค; คลิปไม่มีเลขไซซ์ที่สวม",
      sources: ["https://www.youtube.com/watch?v=QIDUENxVQdA"],
    },
    {
      id: "papziza-puma-deviate-nitro-4",
      reviewerId: "papziza",
      shoeId: "puma-deviate-nitro-4",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "Papziza ทดลอง PUMA Deviate NITRO 4 ที่ US Men's 9 ในระยะสั้นประมาณ 3 กม. และระบุว่า fit ตรงไซส์; ระยะทดลองยังไม่พอสำหรับสรุป durability หรือ fit ระยะยาว",
      sources: ["https://www.youtube.com/watch?v=1pUCMw_XiMs"],
    },
    {
      id: "papziza-pan-equiptech-plus",
      reviewerId: "papziza",
      shoeId: "pan-equiptech-plus",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 1,
        basis: "explicit_delta",
      },
      summary:
        "Papziza ทดลอง PAN Equiptech+ ที่ US Men's 10 และให้คำแนะนำเผื่อขึ้น 1 size; คลิปตั้งข้อสังเกตเรื่อง heel lock และการรองรับเท้าล้ม รวมถึงไม่มี half sizes",
      sources: ["https://www.youtube.com/watch?v=AcMRhMZ6L3A"],
    },
    {
      id: "wing-yang-ngai-hai-uan-anta-pg7-travel-3",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "anta-pg7-travel-3",
      triedSize: {
        system: "US_M",
        value: "11",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "วิ่งยังไงให้อ้วน ทดลอง ANTA PG7 Travel 3 ที่ US Men's 11 และรายงานว่า fit ตรงไซส์; ตัวเลข stack/drop ในคลิปเป็นการประเมิน จึงไม่นำมาเป็นสเปก catalog",
      sources: ["https://www.youtube.com/watch?v=xqji0_K7CaY"],
    },
    {
      id: "wing-yang-ngai-hai-uan-mizuno-neo-vista-3",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "mizuno-neo-vista-3",
      triedSize: {
        system: "US_M",
        value: "11.5",
      },
      summary:
        "วิ่งยังไงให้อ้วน ทดลอง Mizuno Neo Vista 3 ที่ US Men's 11.5 และรายงานประสบการณ์หลังวิ่งประมาณ 15 กม.; durability ระยะยาวยังไม่ถูกทดสอบ",
      sources: ["https://www.youtube.com/watch?v=K47ZtUQG0hk"],
    },
    {
      id: "wing-yang-ngai-hai-uan-anta-zone-2-85",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "anta-zone-2-85",
      triedSize: {
        system: "US_M",
        value: "11",
      },
      summary:
        "วิ่งยังไงให้อ้วน ทดลอง ANTA ZONE 2 85 ที่ US Men's 11 และใช้งานถึงประมาณ 10 กม.; คลิปตั้งข้อสังเกตเรื่องการเกาะบน gravel หรือพื้นเปียก แต่ไม่ได้ให้ size-step recommendation",
      sources: ["https://www.youtube.com/watch?v=8qXMHayH66c"],
    },
    {
      id: "wing-yang-ngai-hai-uan-asics-superblast-3",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "asics-superblast-3",
      triedSize: {
        system: "US_M",
        value: "12",
      },
      summary:
        "วิ่งยังไงให้อ้วน ทดลอง ASICS SUPERBLAST 3 ที่ US Men's 12 และบอกว่าผู้ที่ใช้ Superblast อยู่สามารถคงไซซ์เดิมได้; คำแนะนำครึ่งไซซ์จากความยาวเท้าไม่ถูกแปลงเป็น integer step",
      sources: ["https://www.youtube.com/watch?v=B8-GeOwtDnA"],
    },
    {
      id: "wing-yang-ngai-hai-uan-puma-deviate-pure-nitro",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "puma-deviate-pure-nitro",
      triedSize: {
        system: "US_M",
        value: "12",
      },
      summary:
        "วิ่งยังไงให้อ้วน รีวิว PUMA Deviate Pure NITRO ที่ US Men's 12 และพูดถึงคำแนะนำขยับประมาณครึ่งไซซ์; เนื่องจาก half-size ไม่อยู่ใน integer step จึงเก็บเป็นข้อสังเกตและไม่สร้าง recommendation field",
      sources: ["https://www.youtube.com/watch?v=AYcb8nrkU5w"],
    },
    {
      id: "wing-yang-ngai-hai-uan-new-balance-fuelcell-supercomp-elite-v6",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "new-balance-fuelcell-supercomp-elite-v6",
      summary:
        "วิ่งยังไงให้อ้วน รีวิว New Balance FuelCell SuperComp Elite v6 และระบุให้ยึดไซซ์เดียวกับ V5 พร้อมข้อสังเกตว่าเท้ากว้างอาจต้องเผื่อครึ่งไซซ์; ไม่มีเลขไซซ์ที่สวมแบบยืนยันได้ จึงไม่เติม tried size หรือ step",
      sources: ["https://www.youtube.com/watch?v=LW6jj_-JBIA"],
    },
    {
      id: "wing-yang-ngai-hai-uan-mizuno-wave-rider-30",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "mizuno-wave-rider-30",
      triedSize: {
        system: "US_M",
        value: "11.5",
      },
      summary:
        "วิ่งยังไงให้อ้วน ทดลอง Mizuno Wave Rider 30 ที่ US Men's 11.5 และเล่าประสบการณ์จาก City Run ระยะสั้น; ความทนระยะยาวยังไม่ยืนยัน",
      sources: ["https://www.youtube.com/watch?v=SNk4nXbYruY"],
    },
    {
      id: "por-vrr-adidas-adizero-takumi-sen-11",
      reviewerId: "por-vrr",
      shoeId: "adidas-adizero-takumi-sen-11",
      summary:
        "Por VRR รีวิว adidas Adizero Takumi Sen 11 ว่าเป็น racing flat ที่เบาและเหมาะกับระยะสั้น, tempo และ interval; คลิปพูดถึงความยาว 280 มม. แต่ไม่ได้ผูกตัวเลขนั้นกับระบบ US/UK/EU ที่ชัดเจน จึงไม่เติม tried size หรือ size-step recommendation",
      sources: ["https://www.youtube.com/watch?v=ui08yV4jZwM"],
    },
    {
      id: "por-vrr-adidas-hyperboost-edge",
      reviewerId: "por-vrr",
      shoeId: "adidas-hyperboost-edge",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      summary:
        "Por VRR รีวิว adidas HYPERBOOST EDGE ที่ US Men's 10; ชื่อใน transcription มี ASR variant เป็น HyperBoost Age แต่ official product identity ใช้ HYPERBOOST EDGE. ฟีลแน่นนุ่มเด้งและมั่นคง เหมาะกับ easy/tempo และระยะประมาณ 5–10 กม.",
      sources: ["https://www.youtube.com/watch?v=ZpKwAND1JSc"],
    },
    {
      id: "papziza-norda-000-x-black-diamond",
      reviewerId: "papziza",
      shoeId: "norda-000-x-black-diamond",
      triedSize: {
        system: "US_M",
        value: "10",
      },
      recommendation: {
        stepDelta: 1,
        basis: "explicit_delta",
      },
      summary:
        "Papziza ลองสวม norda 000 x Black Diamond ที่ US Men's 10 ก่อนใช้งานจริง และแนะนำเผื่อ 1 US size จากรองเท้าวิ่งปกติ. นี่เป็น tried-size/recommendation evidence จากการลองสวมเท่านั้น ยังไม่มี running validation จึงไม่เรียก US 10 ว่า Fit Size",
      sources: ["https://www.youtube.com/watch?v=HgPrW-wtIEA&t=11s"],
    },
    {
      id: "papziza-on-cloudboom-strike-2",
      reviewerId: "papziza",
      shoeId: "on-cloudboom-strike-2",
      triedSize: {
        system: "US_M",
        value: "9",
      },
      recommendation: {
        stepDelta: 0,
        basis: "usual_size",
      },
      summary:
        "Papziza วิ่งจริงประมาณ 10 กม. ใน On Cloudboom Strike 2 ที่ US Men's 9 (พูดถึง 27 ซม. ประกอบ) และรายงานว่าฟิตพอดี ล็อกข้อเท้าดี นุ่มเด้งและมั่นคง; คำแนะนำคือใช้ไซซ์ปกติของเขา",
      sources: ["https://www.youtube.com/watch?v=ZgFY4lUOUqM&t=51s"],
    },
    {
      id: "papziza-new-balance-supercomp-rebel-v1",
      reviewerId: "papziza",
      shoeId: "new-balance-supercomp-rebel-v1",
      recommendation: {
        stepDelta: 1,
        basis: "explicit_delta",
      },
      summary:
        "Papziza แกะกล่องและลองสวม New Balance SuperComp Rebel v1; ฟีลนุ่มแน่นและซิ่ง แต่หน้าเท้าแคบและมีแรงดันบริเวณอุ้งเท้า. คลิปแนะนำให้เพิ่ม 0.5 size แต่ยังไม่ได้วิ่งจริงและไม่ได้ยืนยัน tried size จึงไม่สร้าง Fit Size",
      sources: ["https://www.youtube.com/watch?v=V38z1o7ZzjE"],
    },
    {
      id: "wing-yang-ngai-hai-uan-apex-swift-2-0-pro",
      reviewerId: "wing-yang-ngai-hai-uan",
      shoeId: "apex-swift-2-0-pro",
      triedSize: {
        system: "US_M",
        value: "11.5",
      },
      summary:
        "วิ่งยังไงให้อ้วน รีวิว Apex Swift 2.0 Pro โดยระบุ tried size US Men's 11.5 และพูดถึงความยาว 29.5 ซม.; ฟีลโฟมนุ่มเด้งและแผ่นคาร์บอนตอบสนองเมื่อเร่ง แต่แผ่นอาจรู้สึกแข็งเมื่อวิ่งช้า. คลิปไม่มี size recommendation จึงไม่เติม stepDelta หรือสรุปเป็นไซซ์สากล",
      sources: ["https://www.youtube.com/watch?v=XXg-G-k3LRY"],
    },
  ],
  sizeCharts: [
    {
      id: "unpause-project-b-unisex",
      brand: "Unpause",
      audience: "unisex",
      sourceUrl:
        "https://www.facebook.com/unpauseshoes/posts/unpause-project-b%E0%B9%80%E0%B8%95%E0%B8%A3%E0%B8%B5%E0%B8%A2%E0%B8%A1%E0%B8%9E%E0%B8%9A%E0%B8%81%E0%B8%B1%E0%B8%9A-high-performance-racing-shoes-%E0%B8%97%E0%B8%B5%E0%B9%88%E0%B8%A1%E0%B8%AD%E0%B8%9A%E0%B8%82%E0%B8%B5%E0%B8%94%E0%B8%AA%E0%B8%B8%E0%B8%94%E0%B8%82%E0%B8%AD%E0%B8%87%E0%B8%84%E0%B8%A7%E0%B8%B2%E0%B8%A1%E0%B9%80%E0%B8%9A/1592972202828872/",
      // User-supplied official chart image confirms these visible rows for Unpause Project B.
      rows: [
        { US_M: "7", EU: "40", UK: "6", CM: "25" },
        { US_M: "8", EU: "41", UK: "7", CM: "26" },
        { US_M: "8.5", EU: "42", UK: "7.5", CM: "26.5" },
        { US_M: "9", EU: "42.5", UK: "8", CM: "27" },
        { US_M: "9.5", EU: "43", UK: "8.5", CM: "27.5" },
        { US_M: "10", EU: "44", UK: "9", CM: "28" },
        { US_M: "10.5", EU: "44.5", UK: "9.5", CM: "28.5" },
        { US_M: "11", EU: "45", UK: "10", CM: "29" },
      ],
    },
    {
      id: "topo-athletic-men",
      brand: "Topo Athletic",
      audience: "men",
      sourceUrl: "https://www.topoathletic.com/sizing",
      // Topo Athletic's official men's sizing image provides US, EU, UK, and CM labels.
      rows: [
        { US_M: "7", EU: "40", UK: "6", CM: "25" },
        { US_M: "7.5", EU: "40.5", UK: "6.5", CM: "25.5" },
        { US_M: "8", EU: "41", UK: "7", CM: "26" },
        { US_M: "8.5", EU: "42", UK: "7.5", CM: "26.5" },
        { US_M: "9", EU: "42.5", UK: "8", CM: "27" },
        { US_M: "9.5", EU: "43", UK: "8.5", CM: "27.5" },
        { US_M: "10", EU: "44", UK: "9", CM: "28" },
        { US_M: "10.5", EU: "44.5", UK: "9.5", CM: "28.5" },
        { US_M: "11", EU: "45", UK: "10", CM: "29" },
        { US_M: "11.5", EU: "46", UK: "10.5", CM: "29.5" },
        { US_M: "12", EU: "46.5", UK: "11", CM: "30" },
        { US_M: "12.5", EU: "47", UK: "11.5", CM: "30.5" },
        { US_M: "13", EU: "48", UK: "12", CM: "31" },
        { US_M: "14", EU: "49", UK: "13", CM: "32" },
        { US_M: "15", EU: "50", UK: "14", CM: "33" },
      ],
    },
    {
      id: "puma-men",
      brand: "PUMA",
      audience: "men",
      sourceUrl: "https://th.puma.com/th/th/size-guide.html",
      // PUMA's official men's shoe table provides US and UK labels.
      rows: [
        { US_M: "6", UK: "5" },
        { US_M: "6.5", UK: "5.5" },
        { US_M: "7", UK: "6" },
        { US_M: "7.5", UK: "6.5" },
        { US_M: "8", UK: "7" },
        { US_M: "8.5", UK: "7.5" },
        { US_M: "9", UK: "8" },
        { US_M: "9.5", UK: "8.5" },
        { US_M: "10", UK: "9" },
        { US_M: "10.5", UK: "9.5" },
        { US_M: "11", UK: "10" },
        { US_M: "11.5", UK: "10.5" },
        { US_M: "12", UK: "11" },
        { US_M: "12.5", UK: "11.5" },
        { US_M: "13", UK: "12" },
        { US_M: "14", UK: "13" },
        { US_M: "15", UK: "14" },
        { US_M: "16", UK: "15" },
      ],
    },
    {
      id: "asics-men-unisex",
      brand: "ASICS",
      audience: "unisex",
      sourceUrl:
        "https://www.asics.com/th/en-th/customer-service/size-fit-guide/size-fit-guide.html",
      // ASICS's official men's/unisex table uses labels such as 4H for half sizes.
      rows: [
        { US_M: "4", EU: "36", CM: "22.5", UK: "3" },
        { US_M: "4H", EU: "37", CM: "23", UK: "3.5" },
        { US_M: "5", EU: "37.5", CM: "23.5", UK: "4" },
        { US_M: "5H", EU: "38", CM: "24", UK: "4.5" },
        { US_M: "6", EU: "39", CM: "24.5", UK: "5" },
        { US_M: "6H", EU: "39.5", CM: "25", UK: "5.5" },
        { US_M: "7", EU: "40", CM: "25.5", UK: "6" },
        { US_M: "7H", EU: "40.5", CM: "25.75", UK: "6.5" },
        { US_M: "8", EU: "41.5", CM: "26", UK: "7" },
        { US_M: "8H", EU: "42", CM: "26.5", UK: "7.5" },
        { US_M: "9", EU: "42.5", CM: "27", UK: "8" },
        { US_M: "9H", EU: "43.5", CM: "27.5", UK: "8.5" },
        { US_M: "10", EU: "44", CM: "28", UK: "9" },
        { US_M: "10H", EU: "44.5", CM: "28.25", UK: "9.5" },
        { US_M: "11", EU: "45", CM: "28.5", UK: "10" },
        { US_M: "11H", EU: "46", CM: "29", UK: "10.5" },
        { US_M: "12", EU: "46.5", CM: "29.5", UK: "11" },
        { US_M: "12H", EU: "47", CM: "30", UK: "11.5" },
        { US_M: "13", EU: "48", CM: "30.5", UK: "12" },
        { US_M: "13H", EU: "48.5", CM: "30.75", UK: "12.5" },
        { US_M: "14", EU: "49", CM: "31", UK: "13" },
        { US_M: "14H", EU: "49.5", CM: "31.5", UK: "13.5" },
        { US_M: "15", EU: "50.5", CM: "32", UK: "14" },
      ],
    },
    {
      id: "hoka-men-footwear-wide",
      brand: "HOKA",
      audience: "men",
      sourceUrl: "https://vn.hoka.com/pages/size-guide",
      // HOKA's official Men / Footwear / Wide chart. Width-only columns are omitted.
      rows: [
        { US_M: "5", UK: "4.5", EU: "37 1/3", CM: "23" },
        { US_M: "5.5", UK: "5", EU: "38", CM: "23.5" },
        { US_M: "6", UK: "5.5", EU: "38 2/3", CM: "24" },
        { US_M: "6.5", UK: "6", EU: "39 1/3", CM: "24.5" },
        { US_M: "7", UK: "6.5", EU: "40", CM: "25" },
        { US_M: "7.5", UK: "7", EU: "40 2/3", CM: "25.5" },
        { US_M: "8", UK: "7.5", EU: "41 1/3", CM: "26" },
        { US_M: "8.5", UK: "8", EU: "42", CM: "26.5" },
        { US_M: "9", UK: "8.5", EU: "42 2/3", CM: "27" },
        { US_M: "9.5", UK: "9", EU: "43 1/3", CM: "27.5" },
        { US_M: "10", UK: "9.5", EU: "44", CM: "28" },
        { US_M: "10.5", UK: "10", EU: "44 2/3", CM: "28.5" },
        { US_M: "11", UK: "10.5", EU: "45 1/3", CM: "29" },
        { US_M: "11.5", UK: "11", EU: "46", CM: "29.5" },
        { US_M: "12", UK: "11.5", EU: "46 2/3", CM: "30" },
        { US_M: "12.5", UK: "12", EU: "47 1/3", CM: "30.5" },
        { US_M: "13", UK: "12.5", EU: "48", CM: "31" },
        { US_M: "13.5", UK: "13", EU: "48 2/3", CM: "31.5" },
        { US_M: "14", UK: "13.5", EU: "49 1/3", CM: "32" },
        { US_M: "14.5", UK: "14", EU: "50", CM: "32.5" },
        { US_M: "15", UK: "14.5", EU: "50 2/3", CM: "33" },
      ],
    },
    {
      id: "361-men-footwear",
      brand: "361°",
      audience: "men",
      sourceUrl: "https://361sport.com/pages/size-chart",
      // The official chart's CM column contains repeated labels; retain the unambiguous US/UK/EU columns.
      rows: [
        { US_M: "4", UK: "3.5", EU: "36" },
        { US_M: "5", UK: "4", EU: "37.5" },
        { US_M: "5.5", UK: "4.5", EU: "38" },
        { US_M: "6", UK: "5", EU: "38.5" },
        { US_M: "6.5", UK: "5.5", EU: "39" },
        { US_M: "7", UK: "6", EU: "40" },
        { US_M: "7.5", UK: "6.5", EU: "40.5" },
        { US_M: "8", UK: "7", EU: "41" },
        { US_M: "8.5", UK: "7.5", EU: "42" },
        { US_M: "9", UK: "8", EU: "42.5" },
        { US_M: "9.5", UK: "8.5", EU: "43" },
        { US_M: "10", UK: "9", EU: "44" },
        { US_M: "10.5", UK: "9.5", EU: "44.5" },
        { US_M: "11", UK: "10", EU: "45" },
        { US_M: "11.5", UK: "10.5", EU: "45.5" },
        { US_M: "12", UK: "11", EU: "46" },
        { US_M: "12.5", UK: "11.5", EU: "47" },
        { US_M: "13", UK: "12", EU: "47.5" },
        { US_M: "13.5", UK: "12.5", EU: "48" },
        { US_M: "14", UK: "13", EU: "48.5" },
        { US_M: "14.5", UK: "13.5", EU: "49" },
        { US_M: "15", UK: "14", EU: "49.5" },
        { US_M: "15.5", UK: "14.5", EU: "50" },
      ],
    },
    {
      id: "nike-men-footwear",
      brand: "Nike",
      audience: "men",
      sourceUrl: "https://www.nike.com/size-fit/mens-footwear",
      // Nike labels its measurement column CM / JP; it is intentionally not relabelled as CM here.
      rows: [
        { US_M: "3.5", EU: "35.5" },
        { US_M: "4", EU: "36" },
        { US_M: "4.5", EU: "36.5" },
        { US_M: "5", EU: "37.5" },
        { US_M: "5.5", EU: "38" },
        { US_M: "6", EU: "38.5" },
        { US_M: "6.5", EU: "39" },
        { US_M: "7", EU: "40" },
        { US_M: "7.5", EU: "40.5" },
        { US_M: "8", EU: "41" },
        { US_M: "8.5", EU: "42" },
        { US_M: "9", EU: "42.5" },
        { US_M: "9.5", EU: "43" },
        { US_M: "10", EU: "44" },
        { US_M: "10.5", EU: "44.5" },
        { US_M: "11", EU: "45" },
        { US_M: "11.5", EU: "45.5" },
        { US_M: "12", EU: "46" },
        { US_M: "12.5", EU: "47" },
        { US_M: "13", EU: "47.5" },
        { US_M: "13.5", EU: "48" },
        { US_M: "14", EU: "48.5" },
        { US_M: "14.5", EU: "49" },
        { US_M: "15", EU: "49.5" },
        { US_M: "15.5", EU: "50" },
        { US_M: "16", EU: "50.5" },
        { US_M: "16.5", EU: "51" },
        { US_M: "17", EU: "51.5" },
        { US_M: "17.5", EU: "52" },
        { US_M: "18", EU: "52.5" },
        { US_M: "18.5", EU: "53" },
        { US_M: "19", EU: "53.5" },
        { US_M: "19.5", EU: "54" },
        { US_M: "20", EU: "54.5" },
        { US_M: "20.5", EU: "55" },
        { US_M: "21", EU: "55.5" },
        { US_M: "21.5", EU: "56" },
        { US_M: "22", EU: "56.5" },
      ],
    },
    {
      id: "mizuno-men",
      brand: "Mizuno",
      audience: "men",
      sourceUrl: "https://vnm.mizuno.com/pages/size-chart",
      // Mizuno Vietnam's official shoes size chart provides Men's US, UK, EU, and Japan (CM).
      rows: [
        { US_M: "4.5", UK: "3.5", EU: "36", CM: "22.5" },
        { US_M: "5", UK: "4", EU: "36.5", CM: "23" },
        { US_M: "5.5", UK: "4.5", EU: "37", CM: "23.5" },
        { US_M: "6", UK: "5", EU: "38", CM: "24" },
        { US_M: "6.5", UK: "5.5", EU: "38.5", CM: "24.5" },
        { US_M: "7", UK: "6", EU: "39", CM: "25" },
        { US_M: "7.5", UK: "6.5", EU: "40", CM: "25.5" },
        { US_M: "8", UK: "7", EU: "40.5", CM: "26" },
        { US_M: "8.5", UK: "7.5", EU: "41", CM: "26.5" },
        { US_M: "9", UK: "8", EU: "42", CM: "27" },
        { US_M: "9.5", UK: "8.5", EU: "42.5", CM: "27.5" },
        { US_M: "10", UK: "9", EU: "43", CM: "28" },
        { US_M: "10.5", UK: "9.5", EU: "44", CM: "28.5" },
        { US_M: "11", UK: "10", EU: "44.5", CM: "29" },
        { US_M: "11.5", UK: "10.5", EU: "45", CM: "29.5" },
        { US_M: "12", UK: "11", EU: "46", CM: "30" },
        { US_M: "12.5", UK: "11.5", EU: "46.5", CM: "30.5" },
        { US_M: "13", UK: "12", EU: "47", CM: "31" },
        { US_M: "13.5", UK: "12.5", EU: "48", CM: "31.5" },
        { US_M: "14", UK: "13", EU: "48.5", CM: "32" },
      ],
    },
    {
      id: "anta-men",
      brand: "ANTA",
      audience: "men",
      sourceUrl: "https://anta.com/pages/anta-shoes-size-guide",
      // ANTA's official men's shoes size guide provides US, EUR, UK, and Foot Length (cm).
      // The source repeats some UK labels, so this chart retains the unambiguous US/EU/CM columns only.
      rows: [
        { US_M: "4", EU: "36", CM: "22.6" },
        { US_M: "5", EU: "37.5", CM: "23.4" },
        { US_M: "5.5", EU: "38", CM: "23.8" },
        { US_M: "6.5", EU: "39", CM: "24.2" },
        { US_M: "7", EU: "40", CM: "24.7" },
        { US_M: "7.5", EU: "40.5", CM: "25.1" },
        { US_M: "8", EU: "41", CM: "25.5" },
        { US_M: "8.5", EU: "42", CM: "25.9" },
        { US_M: "9", EU: "42.5", CM: "26.3" },
        { US_M: "9.5", EU: "43", CM: "26.8" },
        { US_M: "10", EU: "44", CM: "27.2" },
        { US_M: "10.5", EU: "44.5", CM: "27.6" },
        { US_M: "11", EU: "45", CM: "28" },
        { US_M: "11.5", EU: "45.5", CM: "28.4" },
        { US_M: "12", EU: "46", CM: "28.9" },
        { US_M: "12.5", EU: "47", CM: "29.3" },
        { US_M: "13", EU: "47.5", CM: "29.7" },
        { US_M: "13.5", EU: "48", CM: "29.9" },
        { US_M: "14", EU: "48.5", CM: "30.5" },
        { US_M: "15", EU: "49.5", CM: "31.4" },
        { US_M: "16", EU: "50.5", CM: "32.3" },
        { US_M: "17", EU: "51.5", CM: "31.2" },
      ],
    },
    {
      id: "brooks-men",
      brand: "Brooks",
      audience: "men",
      sourceUrl:
        "https://www.brooksrunning.com/en_us/main-size-guide/?sizeChartIdNew=size-men%3Asize-men-shoes",
      // Brooks's official men's shoe size guide exposes US, UK, and Europe labels.
      rows: [
        { US_M: "5", UK: "4", EU: "37.5" },
        { US_M: "5.5", UK: "4.5", EU: "38" },
        { US_M: "6", UK: "5", EU: "38.5" },
        { US_M: "6.5", UK: "5.5", EU: "39" },
        { US_M: "7", UK: "6", EU: "40" },
        { US_M: "7.5", UK: "6.5", EU: "40.5" },
        { US_M: "8", UK: "7", EU: "41" },
        { US_M: "8.5", UK: "7.5", EU: "42" },
        { US_M: "9", UK: "8", EU: "42.5" },
        { US_M: "9.5", UK: "8.5", EU: "43" },
        { US_M: "10", UK: "9", EU: "44" },
        { US_M: "10.5", UK: "9.5", EU: "44.5" },
        { US_M: "11", UK: "10", EU: "45" },
        { US_M: "11.5", UK: "10.5", EU: "45.5" },
        { US_M: "12", UK: "11", EU: "46" },
        { US_M: "12.5", UK: "11.5", EU: "46.5" },
        { US_M: "13", UK: "12", EU: "47.5" },
        { US_M: "14", UK: "13", EU: "48.5" },
        { US_M: "15", UK: "14", EU: "49.5" },
        { US_M: "16", UK: "15", EU: "50.5" },
      ],
    },
    {
      id: "xtep-men",
      brand: "XTEP",
      audience: "men",
      sourceUrl:
        "https://www.globalxtep.com/xtep-professional-sports-fashion-brand-160x-5-0-pro-product/",
      // XTEP's official men's size chart image provides US, UK, and EUR labels; the source's MM ranges are omitted rather than relabelled as CM.
      rows: [
        { US_M: "6.5", UK: "5.5", EU: "39" },
        { US_M: "7", UK: "6", EU: "39.5" },
        { US_M: "7.5", UK: "6.5", EU: "40" },
        { US_M: "8", UK: "7", EU: "41" },
        { US_M: "8.5", UK: "7.5", EU: "41.5" },
        { US_M: "9", UK: "8", EU: "42" },
        { US_M: "9.5", UK: "8.5", EU: "43" },
        { US_M: "10", UK: "9", EU: "43.5" },
        { US_M: "10.5", UK: "9.5", EU: "44" },
        { US_M: "11", UK: "10", EU: "45" },
      ],
    },
    {
      id: "qiaodan-men",
      brand: "QIAODAN",
      audience: "men",
      sourceUrl: "https://qiaodan.asia/pages/size-guide",
      rows: [
        { US_M: "6.5", EU: "39" },
        { US_M: "7", EU: "40" },
        { US_M: "7.5", EU: "40.5" },
        { US_M: "8", EU: "41" },
        { US_M: "8.5", EU: "42" },
        { US_M: "9", EU: "42.5" },
        { US_M: "9.5", EU: "43" },
        { US_M: "10", EU: "44" },
        { US_M: "10.5", EU: "44.5" },
        { US_M: "11", EU: "45" },
        { US_M: "12", EU: "46" },
        { US_M: "12.5", EU: "47" },
        { US_M: "13", EU: "47.5" },
      ],
    },
    {
      id: "saucony-men",
      brand: "Saucony",
      audience: "men",
      sourceUrl: "https://www.saucony.com/en/size-charts",
      rows: [
        { US_M: "4.5", UK: "3.5", EU: "36", CM: "22.5" },
        { US_M: "5", UK: "4", EU: "37", CM: "23" },
        { US_M: "5.5", UK: "4.5", EU: "38", CM: "24" },
        { US_M: "6", UK: "5", EU: "38.5", CM: "24.5" },
        { US_M: "6.5", UK: "5.5", EU: "39", CM: "25" },
        { US_M: "7", UK: "6", EU: "40", CM: "25.5" },
        { US_M: "7.5", UK: "6.5", EU: "40.5", CM: "26" },
        { US_M: "8", UK: "7", EU: "41", CM: "26.5" },
        { US_M: "8.5", UK: "7.5", EU: "42", CM: "27" },
        { US_M: "9", UK: "8", EU: "42.5", CM: "27.5" },
        { US_M: "9.5", UK: "8.5", EU: "43", CM: "28" },
        { US_M: "10", UK: "9", EU: "44", CM: "28.5" },
        { US_M: "10.5", UK: "9.5", EU: "44.5", CM: "29" },
        { US_M: "11", UK: "10", EU: "45", CM: "29.5" },
        { US_M: "11.5", UK: "10.5", EU: "46", CM: "30" },
        { US_M: "12", UK: "11", EU: "46.5", CM: "30.5" },
        { US_M: "12.5", UK: "11.5", EU: "47", CM: "31" },
        { US_M: "13", UK: "12", EU: "48", CM: "31.5" },
        { US_M: "13.5", UK: "12.5", EU: "49", CM: "32" },
        { US_M: "14", UK: "13", EU: "50", CM: "32.5" },
        { US_M: "14.5", UK: "13.5", EU: "51", CM: "33" },
        { US_M: "15", UK: "14", EU: "52", CM: "33.5" },
      ],
    },
    {
      id: "salomon-men",
      brand: "Salomon",
      audience: "men",
      sourceUrl: "https://sg.salomon.com/pages/men-size-guide",
      rows: [
        { US_M: "4M", UK: "3.5", EU: "36", CM: "21,5" },
        { US_M: "4.5 M", UK: "4", EU: "36 2/3", CM: "22" },
        { US_M: "5 M", UK: "4.5", EU: "37 1/3", CM: "22,5" },
        { US_M: "5.5 M", UK: "5", EU: "38", CM: "23" },
        { US_M: "6 M", UK: "5.5", EU: "38 2/3", CM: "23,5" },
        { US_M: "6.5 M", UK: "6", EU: "39 1/3", CM: "24" },
        { US_M: "7 M", UK: "6.5", EU: "40", CM: "24,5" },
        { US_M: "7.5 M", UK: "7", EU: "40 2/3", CM: "25" },
        { US_M: "8 M", UK: "7.5", EU: "41 1/3", CM: "25,5" },
        { US_M: "8.5 M", UK: "8", EU: "42", CM: "26" },
        { US_M: "9 M", UK: "8.5", EU: "42 2/3", CM: "26,5" },
        { US_M: "9.5 M", UK: "9", EU: "43 1/3", CM: "27" },
        { US_M: "10 M", UK: "9.5", EU: "44", CM: "27,5" },
        { US_M: "10.5 M", UK: "10", EU: "44 2/3", CM: "28" },
        { US_M: "11 M", UK: "10.5", EU: "45 1/3", CM: "28,5" },
        { US_M: "11.5 M", UK: "11", EU: "46", CM: "29" },
        { US_M: "12 M", UK: "11.5", EU: "46 2/3", CM: "29,5" },
        { US_M: "12.5 M", UK: "12", EU: "47 1/3", CM: "30" },
        { US_M: "13 M", UK: "12.5", EU: "48", CM: "30,5" },
        { US_M: "13.5 M", UK: "13", EU: "48 2/3", CM: "31" },
        { US_M: "14 M", UK: "13.5", EU: "49 1/3", CM: "31,5" },
        { US_M: "14.5 M", UK: "14", EU: "50", CM: "32" },
        { US_M: "15 M", UK: "14.5", EU: "50 2/3", CM: "32,5" },
      ],
    },
    {
      id: "new-balance-men-unisex",
      brand: "New Balance",
      audience: "unisex",
      sourceUrl: "https://www.newbalance.com/size-guide.html",
      rows: [
        { US_M: "2.5", UK: "2", EU: "34", CM: "20.5" },
        { US_M: "3", UK: "2.5", EU: "35", CM: "21" },
        { US_M: "3.5", UK: "3", EU: "35.5", CM: "21.5" },
        { US_M: "4", UK: "3.5", EU: "36", CM: "22" },
        { US_M: "4.5", UK: "4", EU: "37", CM: "22.5" },
        { US_M: "5", UK: "4.5", EU: "37.5", CM: "23" },
        { US_M: "5.5", UK: "5", EU: "38", CM: "23.5" },
        { US_M: "6", UK: "5.5", EU: "38.5", CM: "24" },
        { US_M: "6.5", UK: "6", EU: "39.5", CM: "24.5" },
        { US_M: "7", UK: "6.5", EU: "40", CM: "25" },
        { US_M: "7.5", UK: "7", EU: "40.5", CM: "25.5" },
        { US_M: "8", UK: "7.5", EU: "41.5", CM: "26" },
        { US_M: "8.5", UK: "8", EU: "42", CM: "26.5" },
        { US_M: "9", UK: "8.5", EU: "42.5", CM: "27" },
        { US_M: "9.5", UK: "9", EU: "43", CM: "27.5" },
        { US_M: "10", UK: "9.5", EU: "44", CM: "28" },
        { US_M: "10.5", UK: "10", EU: "44.5", CM: "28.5" },
        { US_M: "11", UK: "10.5", EU: "45", CM: "29" },
        { US_M: "11.5", UK: "11", EU: "45.5", CM: "29.5" },
        { US_M: "12", UK: "11.5", EU: "46.5", CM: "30" },
        { US_M: "12.5", UK: "12", EU: "47", CM: "30.5" },
        { US_M: "13", UK: "12.5", EU: "47.5", CM: "31" },
        { US_M: "13.5", UK: "13", EU: "48.5", CM: "31.5" },
        { US_M: "14", UK: "13.5", EU: "49", CM: "32" },
        { US_M: "15", UK: "14.5", EU: "50", CM: "33" },
        { US_M: "16", UK: "15.5", EU: "51", CM: "34" },
        { US_M: "17", UK: "16.5", EU: "52", CM: "35" },
        { US_M: "18", UK: "17.5", EU: "53", CM: "36" },
        { US_M: "19", UK: "18.5", EU: "54", CM: "37" },
        { US_M: "20", UK: "19.5", EU: "55", CM: "38" },
      ],
    },
    {
      id: "on-men-unisex",
      brand: "On",
      audience: "men",
      sourceUrl:
        "https://www.on.com/en-us/products/cloudmonster-3-m-3mg1005/mens/ghost-glacier-shoes-3MG10055243",
      rows: [
        { US_M: "7", UK: "6", EU: "40" },
        { US_M: "7.5", UK: "6.5", EU: "40.5" },
        { US_M: "8", UK: "7", EU: "41" },
        { US_M: "8.5", UK: "7.5", EU: "42" },
        { US_M: "9", UK: "8", EU: "42.5" },
        { US_M: "9.5", UK: "8.5", EU: "43" },
        { US_M: "10", UK: "9", EU: "44" },
        { US_M: "10.5", UK: "9.5", EU: "44.5" },
        { US_M: "11", UK: "10", EU: "45" },
        { US_M: "11.5", UK: "10.5", EU: "45.5" },
        { US_M: "12", UK: "11", EU: "46" },
        { US_M: "12.5", UK: "11.5", EU: "47" },
        { US_M: "13", UK: "12", EU: "47.5" },
        { US_M: "13.5", UK: "12.5", EU: "48" },
        { US_M: "14", UK: "13", EU: "49" },
        { US_M: "14.5", UK: "13.5", EU: "49.5" },
        { US_M: "15", UK: "14", EU: "50" },
      ],
    },
    {
      id: "hiracer-men",
      brand: "HIRACER",
      audience: "men",
      sourceUrl: "https://hiracers.com/pages/hiracer-size-charts",
      rows: [
        { US_M: "4", UK: "3", EU: "36", CM: "22" },
        { US_M: "4.5", UK: "3.5", EU: "36.5", CM: "22.5" },
        { US_M: "5", UK: "4", EU: "37.5", CM: "23" },
        { US_M: "5.5", UK: "4.5", EU: "38", CM: "23.5" },
        { US_M: "6", UK: "5", EU: "38.5", CM: "24" },
        { US_M: "6.5", UK: "5.5", EU: "39", CM: "24.5" },
        { US_M: "7", UK: "6", EU: "40", CM: "25" },
        { US_M: "7.5", UK: "6.5", EU: "40.5", CM: "25.5" },
        { US_M: "8", UK: "7", EU: "41", CM: "26" },
        { US_M: "8.5", UK: "7.5", EU: "42", CM: "26.5" },
        { US_M: "9", UK: "8", EU: "42.5", CM: "27" },
        { US_M: "9.5", UK: "8.5", EU: "43", CM: "27.5" },
        { US_M: "10", UK: "9", EU: "44", CM: "28" },
        { US_M: "10.5", UK: "9.5", EU: "44.5", CM: "28.5" },
        { US_M: "11", UK: "10", EU: "45", CM: "29" },
        { US_M: "11.5", UK: "10.5", EU: "45.5", CM: "29.5" },
        { US_M: "12", UK: "11", EU: "46", CM: "30" },
        { US_M: "12.5", UK: "11.5", EU: "47", CM: "30.5" },
        { US_M: "13", UK: "12", EU: "47.5", CM: "31" },
        { US_M: "13.5", UK: "12.5", EU: "48", CM: "31.5" },
        { US_M: "14", UK: "13", EU: "49", CM: "32" },
        { US_M: "14.5", UK: "13.5", EU: "50", CM: "32.5" },
        { US_M: "15", UK: "14", EU: "51", CM: "33" },
      ],
    },
    {
      id: "norda-men",
      brand: "norda",
      audience: "men",
      sourceUrl: "https://nordarun.com/pages/faq",
      rows: [
        { US_M: "M7", UK: "6", EU: "39⅓", CM: "24.5" },
        { US_M: "M7.5", UK: "6.5", EU: "40", CM: "25" },
        { US_M: "M8", UK: "7", EU: "40⅔", CM: "25.25" },
        { US_M: "M8.5", UK: "7.5", EU: "41⅓", CM: "25.75" },
        { US_M: "M9", UK: "8", EU: "42", CM: "26.25" },
        { US_M: "M9.5", UK: "8.5", EU: "42⅔", CM: "26.5" },
        { US_M: "M10", UK: "9", EU: "43⅓", CM: "27" },
        { US_M: "M10.5", UK: "9.5", EU: "44", CM: "27.5" },
        { US_M: "M11", UK: "10", EU: "44⅔", CM: "27.75" },
        { US_M: "M11.5", UK: "10.5", EU: "45⅓", CM: "28.25" },
        { US_M: "M12", UK: "11", EU: "46", CM: "28.5" },
        { US_M: "M12.5", UK: "11.5", EU: "46⅔", CM: "29.5" },
        { US_M: "M13", UK: "12", EU: "47", CM: "29.75" },
        { US_M: "M14", UK: "13", EU: "48⅓", CM: "30.5" },
        { US_M: "M15", UK: "14", EU: "49⅔", CM: "31.25" },
      ],
    },
    {
      // User-supplied Adidas chart image; labels are preserved verbatim.
      id: "adidas-men",
      brand: "adidas",
      audience: "men",
      sourceUrl: "https://www.adidas.com/us/help/size_charts",
      rows: [
        { US_M: "7½", UK: "7", EU: "40⅔", CM: "25.5" },
        { US_M: "8", UK: "7½", EU: "41⅓", CM: "26" },
        { US_M: "8½", UK: "8", EU: "42", CM: "26.5" },
        { US_M: "9", UK: "8½", EU: "42⅔", CM: "27" },
        { US_M: "9½", UK: "9", EU: "43⅓", CM: "27.5" },
        { US_M: "10", UK: "9½", EU: "44", CM: "28" },
        { US_M: "10½", UK: "10", EU: "44⅔", CM: "28.5" },
        { US_M: "11", UK: "10½", EU: "45⅓", CM: "29" },
        { US_M: "11½", UK: "11", EU: "46", CM: "29.5" },
        { US_M: "12", UK: "11½", EU: "46⅔", CM: "30" },
        { US_M: "12½", UK: "12", EU: "47⅓", CM: "30.5" },
        { US_M: "13", UK: "12½", EU: "48", CM: "31" },
      ],
    },
    {
      // JPN is present in the source image but the catalog schema has no JPN key.
      // It is intentionally omitted rather than relabeled as CM.
      id: "apex-unisex",
      brand: "Apex",
      audience: "unisex",
      sourceUrl: "https://www.apexswiftrunning.com/product/apex-swift-20-pro",
      rows: [
        { US_M: "6", UK: "5", EU: "38" },
        { US_M: "6.5", UK: "5.5", EU: "39" },
        { US_M: "7", UK: "6", EU: "40" },
        { US_M: "7.5", UK: "6.5", EU: "40.5" },
        { US_M: "8", UK: "7", EU: "41" },
        { US_M: "8.5", UK: "7.5", EU: "42" },
        { US_M: "9", UK: "8", EU: "42.5" },
        { US_M: "9.5", UK: "8.5", EU: "43" },
        { US_M: "10", UK: "9", EU: "44" },
        { US_M: "10.5", UK: "9.5", EU: "44.5" },
        { US_M: "11", UK: "10", EU: "45" },
        { US_M: "11.5", UK: "10.5", EU: "46" },
        { US_M: "12", UK: "11", EU: "47" },
        { US_M: "12.5", UK: "11.5", EU: "47.5" },
        { US_M: "13", UK: "12", EU: "48.5" },
      ],
    },
  ],
};

/** Build-time validated static catalog. */
export const catalog: Catalog = validateCatalog(productionCatalog);

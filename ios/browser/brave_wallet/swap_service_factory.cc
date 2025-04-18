/* Copyright (c) 2021 The Brave Authors. All rights reserved.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/. */

#include "brave/ios/browser/brave_wallet/swap_service_factory.h"

#include "base/no_destructor.h"
#include "brave/components/brave_wallet/browser/swap_service.h"
#include "components/keyed_service/core/keyed_service.h"
#include "ios/chrome/browser/shared/model/profile/profile_ios.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"

namespace brave_wallet {

// static
mojo::PendingRemote<mojom::SwapService> SwapServiceFactory::GetForProfile(
    ProfileIOS* profile) {
  return GetInstance()
      ->GetServiceForProfileAs<SwapService>(profile, true)
      ->MakeRemote();
}

// static
SwapServiceFactory* SwapServiceFactory::GetInstance() {
  static base::NoDestructor<SwapServiceFactory> instance;
  return instance.get();
}

SwapServiceFactory::SwapServiceFactory()
    : ProfileKeyedServiceFactoryIOS("SwapService",
                                    ProfileSelection::kRedirectedInIncognito,
                                    ServiceCreation::kCreateLazily,
                                    TestingCreation::kNoServiceForTests) {}

SwapServiceFactory::~SwapServiceFactory() = default;

std::unique_ptr<KeyedService> SwapServiceFactory::BuildServiceInstanceFor(
    web::BrowserState* context) const {
  auto* profile = ProfileIOS::FromBrowserState(context);
  std::unique_ptr<SwapService> swap_service(
      new SwapService(profile->GetSharedURLLoaderFactory()));
  return swap_service;
}

}  // namespace brave_wallet

use crate::service;

use codec::{Decode, Encode};
use futures::{
    join, select_biased,
    stream::{FuturesUnordered, StreamExt},
};
use gstd::{msg, prelude::*, sync::RwLock, ActorId};

#[derive(Debug, Decode, Encode)]
pub enum Control {
    GetOwner,
    ReplaceOwner {
        new_owner: ActorId,
    },
    GetFixtures,
    RemoveFixture {
        index: u32,
    },
    UpdateFixture {
        index: u32,
        fixture: service::Fixture,
    },
    AddFixture {
        fixture: service::Fixture,
    },
    ClearFixtures,
    RunFixtures,
}

#[derive(Debug, Decode, Encode)]
pub enum Error {
    NotFound,
    NotEnoughGas { actual: u64, needed: u64 },
}

#[derive(Debug, Decode, Encode)]
pub struct Init {
    pub owner: ActorId,
    pub service_address: ActorId,
}

#[derive(Debug, Decode, Encode)]
pub enum Event {
    FixtureSuccess {
        index: u32,
    },
    FixtureFail {
        index: u32,
        fail_hint: service::StringIndex,
    },
    PreparationFail {
        index: u32,
    },
}

#[derive(Debug)]
pub struct Reply {
    pub payload: Option<Vec<u8>>,
}

impl<T: Encode> From<T> for Reply {
    fn from(t: T) -> Self {
        Reply {
            payload: Some(t.encode()),
        }
    }
}

impl Reply {
    pub fn none() -> Self {
        Reply { payload: None }
    }
}

pub struct Handler<'a> {
    service: &'a RwLock<service::Service>,
    owner: ActorId,
}

#[derive(Default, Debug, Decode, Encode)]
pub struct FailedFixtures {
    pub indices: Vec<(u32, service::StringIndex)>,
}

impl<'a> Handler<'a> {
    pub fn new(service: &'a RwLock<service::Service>, owner: ActorId) -> Self {
        Self { service, owner }
    }

    pub async fn dispatch(&mut self, control: Control) -> Reply {
        use Control::*;
        match control {
            GetOwner => self.get_owner().into(),
            ReplaceOwner { new_owner } => {
                self.owner = new_owner;
                Reply::none()
            }
            GetFixtures => self.get_fixtures().await.into(),
            RemoveFixture { index } => self.remove_fixture(index).await.into(),
            UpdateFixture { index, fixture } => self.update_fixture(index, fixture).await.into(),
            AddFixture { fixture } => {
                self.add_fixture(fixture).await;
                Reply::none()
            }
            ClearFixtures => {
                self.clear_fixtures().await;
                Reply::none()
            }
            RunFixtures => self.run_fixtures().await.into(),
        }
    }

    fn get_owner(&self) -> ActorId {
        self.owner.clone()
    }

    async fn get_fixtures(&self) -> Vec<service::Fixture> {
        self.service.read().await.fixtures().to_vec()
    }

    async fn remove_fixture(&mut self, index: u32) -> Result<(), Error> {
        let mut service = self.service.write().await;
        if (index as usize) < service.fixtures().len() {
            service.drop_fixture(index as usize);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    async fn update_fixture(&mut self, index: u32, fixture: service::Fixture) -> Result<(), Error> {
        let mut service = self.service.write().await;

        if (index as usize) < service.fixtures().len() {
            service.fixtures_mut()[index as usize] = fixture;

            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    async fn add_fixture(&mut self, fixture: service::Fixture) {
        self.service.write().await.add_fixture(fixture);
    }

    async fn clear_fixtures(&mut self) {
        self.service.write().await.clear_fixtures();
    }

    async fn run_fixtures(&self) -> Result<FailedFixtures, Error> {
        enum RuntimeError {
            PreparationSendFail(u32),
            ExpectationSendFail(u32),
            ExpectationExecutionFail(u32, gstd::errors::Error),
            PayloadMismatch(u32, Vec<u8>),
        }

        let sender = msg::source();
        let service = self.service.read().await;

        let gas_required = service.gas_required();
        let gas_available = gstd::exec::gas_available();
        if gas_available < gas_required {
            return Err(Error::NotEnoughGas {
                actual: gas_available,
                needed: gas_required,
            });
        }

        let fails_list: Vec<(u32, service::StringIndex)> = vec![];

        let fixtures_stream = FuturesUnordered::new();

        for fixture_no in 0..service.fixtures().len() {
            let ref_svc = &service; // to do only partial move below
            fixtures_stream.push(async move {
                let fixture = &ref_svc.fixtures()[fixture_no];

                // preparations
                for preparation in fixture.preparation.iter() {
                    let _ = match gstd::msg::send_bytes_for_reply(
                        ref_svc.address(),
                        preparation.payload.clone(),
                        0, // TODO: figure out preparation.value,
                        preparation.gas,
                    ) {
                        Ok(fut) => fut,
                        Err(e) => return Err(RuntimeError::PreparationSendFail(fixture_no as u32)),
                    }
                    .await; // we don't care about what preparation returns
                }

                // expectations
                for expectation in fixture.expectations.iter() {
                    let result = match gstd::msg::send_bytes_for_reply(
                        ref_svc.address(),
                        expectation.request.payload.clone(),
                        0, // TODO: figure out expectation.request.value,
                        expectation.request.gas,
                    ) {
                        Ok(fut) => fut,
                        Err(_) => return Err(RuntimeError::ExpectationSendFail(fixture_no as u32)),
                    }
                    .await;

                    match result {
                        Ok(payload) => {
                            if let Some(expected_payload) = expectation.response.payload.as_ref() {
                                if expected_payload != &payload[..] {
                                    return Err(RuntimeError::PayloadMismatch(
                                        fixture_no as u32,
                                        payload,
                                    ));
                                }
                                // TODO: check gas & value somehow
                            }
                        }
                        Err(e) => {
                            return Err(RuntimeError::ExpectationExecutionFail(
                                fixture_no as u32,
                                e,
                            ));
                        }
                    }
                }

                Ok(())
            });
        }

        Ok(FailedFixtures::default())
    }
}

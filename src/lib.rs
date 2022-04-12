use scrypto::prelude::*;

blueprint! {
    struct Loan {
        cash_vault: Vault,
        loan_interest: Decimal,
        lender_badge: Vault,
        lender_resource: ResourceDef,
    }

    impl Loan {

        pub fn new(loan_interest: Decimal, cash_address: Address) -> Component {
            let admin_badge: Bucket = ResourceBuilder::new_fungible(DIVISIBILITY_NONE).initial_supply_fungible(1);
            let lender_resource_def: ResourceDef = ResourceBuilder::new_fungible(DIVISIBILITY_MAXIMUM)
                .metadata("name", "LenderToken")
                .metadata("symbol", "LT")
                .flags(MINTABLE | BURNABLE)
                .badge(admin_badge.resource_def(), MAY_MINT | MAY_BURN)
                .metadata("description", "A lender token")
                .no_initial_supply();

            Self {
                cash_vault: Vault::new(ResourceDef::from(cash_address)),
                loan_interest: loan_interest,
                lender_resource: lender_resource_def,
                lender_badge: Vault::with_bucket(admin_badge)
            }
            .instantiate()
        }

        // mints new lender tokens at the current exchange rate
        pub fn buy_lenders(&mut self, payment: Bucket) -> Bucket {
            let exchange_rate: Decimal = if self.cash_vault.amount() > Decimal::from(0) { self.lender_resource.total_supply()/self.cash_vault.amount() } else { Decimal::from(1) };
            let lenders_bought: Decimal = exchange_rate*payment.amount();
            self.cash_vault.put(payment);
            self.lender_badge.authorize(|auth|{
                self.lender_resource.mint(lenders_bought, auth)
            })
        }

        // sells lender tokens for cash at the initial rate (or better if the flashloan has been used)
        pub fn sell_lenders(&mut self, lenders: Bucket) -> Bucket {
            let cash_returned: Decimal = (self.cash_vault.amount()/self.lender_resource.total_supply())*lenders.amount();
            self.lender_badge.authorize(|auth|{
                self.lender_resource.burn_with_auth(lenders, auth);
            });
            self.cash_vault.take(cash_returned)
        }


        // flash loan code taken from tweeted repo
        pub fn request_loan(&mut self, amount: Decimal, component_address: Address) -> Bucket {
            assert!(amount < self.cash_vault.amount(), "Not enough funds to loan");

            // Call the execute method at the specified component's address with the requested funds
            let args = vec![
                scrypto_encode(&self.cash_vault.take(amount))
            ];

            let mut returned_bucket: Bucket = Component::from(component_address).call::<Bucket>("execute", args).into();

            // Make sure they repaid in loan in full
            let amount_to_take = amount * ((self.loan_interest / 100) + 1);
            assert!(returned_bucket.amount() >= amount_to_take, "You have to return more than {}", amount_to_take);

            self.cash_vault.put(returned_bucket.take(amount_to_take));

            // Return the change back to the component
            return returned_bucket;
        }

        // lends out all the cash in the vault
        pub fn request_max_loan(&mut self, component_address: Address) -> Bucket {
            self.request_loan(self.cash_vault.amount(),component_address)
        }

        // similar to request_loan but will loan max rather than fail if the amount cannot be filled
        pub fn request_loan_upto(&mut self, amount: Decimal, component_address: Address) -> Bucket {
            self.request_loan(if amount > self.cash_vault.amount(){self.cash_vault.amount()}else{amount}, component_address)
        }

        pub fn get_balance(&self) -> Decimal {
            self.cash_vault.amount()
        }

    }
}

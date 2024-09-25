module Main where

import Prelude

import AllUsers (Query)
import Data.Argonaut (class DecodeJson)
import Data.Symbol (class IsSymbol)
import Data.Variant (Variant, inj)
import Effect (Effect)
import Effect.Aff (Aff, launchAff_)
import GraphQL.Client.Args ((=>>))
import GraphQL.Client.Operation (OpQuery)
import GraphQL.Client.Query (query_)
import GraphQL.Client.Types (class GqlQuery)
import Prim.Row as R
import Type.Data.List (Nil')
import Type.Proxy (Proxy(..))

main :: Effect Unit
main =
  launchAff_
    $ void
    $ queryGql "some_fake_query_to_type_check"
        { export_progresses:
            { distinct_on:
                [ sum @"complete"
                , sum @"id"
                ]
            } =>> { id: unit }
        }

-- Run gql query
queryGql
  :: forall query returns
   . GqlQuery Nil' OpQuery Query query returns
  => DecodeJson returns
  => String
  -> query
  -> Aff returns
queryGql = query_ "http://localhost:4892/graphql" (Proxy :: Proxy Query)

sum :: ∀ @sym r1 r2. R.Cons sym Unit r1 r2 ⇒ IsSymbol sym => Variant r2
sum = inj (Proxy @sym) unit
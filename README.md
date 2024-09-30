# Config for purescript-graphql-schema-gen

## Hasura roles

To generate schemas for each of your Hasura roles, create a .yaml file and add a ROLES_YAML env var.
The yaml should be made up of an array of strings:

```yaml
- Admin
- User
- Submitter
```

## Outside types

To replace default hasura types with Purescript types, create one or more .yaml files and add them - comma separated - to the OUTSIDE_TYPES_YAML env var.

### `outside_types`:

This section is where you define the schema objects that you want to replace values for.

For example if you wanted to change the types on the `user` object, you could add the following:

```yaml
outside_types:
  user:
    with: common
    id: id=UserId
    email: EmailAddress, Data.EmailAddress, emails
```

This would replace the 'id' field with the 'UserId' type (see 'types' section below). It would also replace the 'email' field with an inline type of the format: `[type name], [module name], [package name]`.

The `with` key is an optional special key that allows you to define a common set of types to use across multiple objects. For example, if you wanted to use a 'common' set of types on both the 'user' and 'post' objects, you would define 'common' types in the 'templates' section and then add a `with: common` key to the 'user' and 'post' objects.

### types:

This section is where you define shorthand type templates modules containing multiple types. An example for a couple of `Id` modules is:

```yaml
types:
  id: $, Data.Id.$, oa-ids
  drId: $, Data.Id.DelegateRegistration.$, oa-ids
```

The '$' symbol is a placeholder for the type name that the type is called with in templates or outside_types. For example in the 'outside_types' section above, we use `id=UserId` which is translated into `UserId, Data.Id.UserId, oa-ids`.

### `templates`:

This section is where you define the common types that you want to use across multiple objects via the `with` key. For example:

```yaml
templates
  common:
    author_id: id=AuthorId
    stage_id: id=StageId
    user_id: id=UserId
    created_by: id=UserId
    client_id: id=ClientId
    event_id: id=EventId
  event_types:
    event_title: HTML, Data.HTML
    frequency: Frequency, Data.Frequency
    created_by: id=UserId
    client_id: id=ClientId
    event_id: id=EventId
```

The keys don't have to exist on the object you call the template for, but any keys that do match will be replaced with the template value.

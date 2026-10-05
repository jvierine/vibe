"""Projection-only tests. No scientific model or agent trials are executed."""
import json
import unittest
from render_packs import fields, fence, pseudocode, type_text


class RenderTests(unittest.TestCase):
    def test_escaped_strings_and_nested_annotation(self):
        value = 'line one\n"quoted" \\ path'
        self.assertEqual(fields('value=' + json.dumps(value))['value'], value)
        self.assertEqual(fields('name="x" mutable=true annotation={"Scalar":["f64","m/s"]}')['annotation'], {'Scalar':['f64','m/s']})
        self.assertEqual(type_text({'Scalar':['f64','m/s']}), 'f64 [m/s]')

    def test_no_internal_hashes_or_fake_string_value(self):
        records = {'body/0':('return', {'has_value':'true'}),
                   'body/0/value':('string', {'value':'omitted','hash':'sha256:secret','bytes':'2048'})}
        text = pseudocode(records)
        self.assertIn('string omitted', text)
        self.assertNotIn('sha256', text)

    def test_mutation_units_branches_and_empty_blocks(self):
        records = {
            'body/0':('let', {'name':'x','mutable':'true','annotation':{'Scalar':['f64','m']}}),
            'body/0/value':('number', {'value':'2.0','scalar':'f64','unit':'m'}),
            'body/1':('if', {}), 'body/1/condition':('variable', {'name':'ready'}),
            'body/1/else/0':('return', {'has_value':'false'})}
        text = pseudocode(records)
        self.assertIn('var x: f64 [m] = 2.0 [m]', text)
        self.assertIn('if ready:\n    pass\nelse:\n    return', text)

    def test_safe_markdown_fence_and_unknown_nodes(self):
        self.assertTrue(fence('literal ```').startswith('````text'))
        with self.assertRaises(ValueError): pseudocode({'body/0':('future_node', {})})


if __name__ == '__main__': unittest.main()
